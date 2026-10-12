//! A quick two-player test over UDP: the online menu (F10: hosting on a port, or joining an address) or, for test
//! runs, `AC1_HOST=port` waits for a friend and `AC1_JOIN=ip:port` connects to one. A host takes one friend: the
//! first to send, or a new one once the last has been quiet `PEER_TIMEOUT`. Each side sends its player's root and pose about 30 times a second; the other
//! player shows as a second Altaïr posed from them. No game data crosses the wire, only transforms.
//!
//! Packet: "AC1N", version, root position (3 x f32), root rotation (4 x f32), bone count (u16), then each bone's
//! rotation as 4 x i16 (x 32767), the first `POS_BONES` bones' positions as 3 x f32 (the rest keep the bind pose:
//! both sides read the same skeleton), then the sender's camera, position and rotation (7 x f32: for replays, the
//! friend's point of view).
use bevy::prelude::*;
use std::net::{SocketAddr, UdpSocket};

use crate::character::{Character, Player};

const MAGIC: &[u8; 4] = b"AC1N";
const VERSION: u8 = 2;
/// Bones whose positions are sent too (the root and hips move; the rest are fixed lengths).
const POS_BONES: usize = 4;
/// Seconds between sends.
const SEND_EVERY: f32 = 1.0 / 30.0;
/// The friend is shown while heard within this (s).
const HEARD: f32 = 3.0;
/// Hosting, a friend quiet this long (s) gives their place to whoever sends next.
const PEER_TIMEOUT: f32 = 6.0;

#[derive(Resource)]
pub struct Net {
    sock: UdpSocket,
    /// Who we talk to: given when joining, learned from the first packet when hosting.
    peer: Option<SocketAddr>,
    hosting: bool,
    since_send: f32,
    /// The last state heard: root and bone transforms.
    last: Option<(Vec3, Quat, Vec<Quat>, Vec<Vec3>)>,
    /// The friend's camera, last heard.
    cam: Option<(Vec3, Quat)>,
    heard: f32,
    /// Seconds since the socket opened.
    up: f32,
    /// Our port.
    port: u16,
    /// What went wrong last on receiving (Windows: nobody listening at the address joined).
    error: Option<String>,
}

/// The other player's figure, and the pose last shown on it (smoothed toward what is heard).
#[derive(Component, Default)]
pub struct Remote {
    shown: Option<ik::Pose>,
}

/// Open the socket asked for by `AC1_HOST` or `AC1_JOIN`. (Before the app's logging starts: printed.)
pub fn open() -> Option<Net> {
    let n = if let Ok(port) = std::env::var("AC1_HOST") { host(port.trim().parse().unwrap_or(7777)) } else { join(&std::env::var("AC1_JOIN").ok()?) };
    match n {
        Ok(n) => {
            eprintln!("net: {}", n.status());
            Some(n)
        }
        Err(e) => {
            eprintln!("net: {e}");
            None
        }
    }
}

/// Wait for a friend on UDP `port`.
pub fn host(port: u16) -> Result<Net, String> {
    let s = UdpSocket::bind(("0.0.0.0", port)).map_err(|e| format!("cannot listen on UDP port {port}: {e}"))?;
    Net::new(s, None, true)
}

/// Join a host at `ip:port` (a name works too).
pub fn join(to: &str) -> Result<Net, String> {
    use std::net::ToSocketAddrs;
    let to = to.trim();
    let addr = to
        .to_socket_addrs()
        .map_err(|e| format!("{to} is no address to join ({e}): ip:port"))?
        .find(|a| a.is_ipv4())
        .ok_or_else(|| format!("{to}: no IPv4 address"))?;
    let s = UdpSocket::bind(("0.0.0.0", 0)).map_err(|e| format!("cannot open a UDP socket: {e}"))?;
    Net::new(s, Some(addr), false)
}

impl Net {
    fn new(sock: UdpSocket, peer: Option<SocketAddr>, hosting: bool) -> Result<Self, String> {
        sock.set_nonblocking(true).map_err(|e| format!("socket: {e}"))?;
        let port = sock.local_addr().map_or(0, |a| a.port());
        Ok(Net { sock, peer, hosting, since_send: SEND_EVERY, last: None, cam: None, heard: f32::MAX, up: 0.0, port, error: None })
    }

    pub fn hosting(&self) -> bool {
        self.hosting
    }

    /// Our UDP port.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Players heard now, besides us (0 or 1).
    pub fn peers(&self) -> usize {
        usize::from(self.heard < HEARD)
    }

    /// One line for the menu and the log: listening, connected, how many, what went wrong.
    pub fn status(&self) -> String {
        let peers = self.peers();
        let s = match (self.hosting, self.peer) {
            (true, None) => format!("hosting: listening on UDP port {}, waiting for a friend", self.port),
            (true, Some(p)) if peers > 0 => format!("hosting on UDP port {}: {p} connected", self.port),
            (true, Some(p)) => format!("hosting on UDP port {}: {p} quiet for {:.0} s, waiting", self.port, self.heard),
            (false, Some(p)) if peers > 0 => format!("connected to {p}"),
            (false, Some(p)) if self.heard < f32::MAX => format!("joined {p}: no packets for {:.0} s", self.heard),
            (false, Some(p)) => format!("joining {p}: no answer yet ({:.0} s){}", self.up, self.error.as_ref().map_or(String::new(), |e| format!(": {e}"))),
            (false, None) => "not connected".to_string(),
        };
        format!("{s} ({peers} peer{})", if peers == 1 { "" } else { "s" })
    }

    /// The friend's camera, while they are heard.
    pub fn friend_camera(&self) -> Option<(Vec3, Quat)> {
        self.cam.filter(|_| self.heard < HEARD)
    }
}

fn encode(root: &Transform, pose: &ik::Pose, cam: Option<&Transform>) -> Vec<u8> {
    let mut b = Vec::with_capacity(40 + pose.local.len() * 8 + POS_BONES * 12);
    b.extend_from_slice(MAGIC);
    b.push(VERSION);
    for v in [root.translation.x, root.translation.y, root.translation.z, root.rotation.x, root.rotation.y, root.rotation.z, root.rotation.w] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&(pose.local.len() as u16).to_le_bytes());
    for x in &pose.local {
        for v in [x.rot.x, x.rot.y, x.rot.z, x.rot.w] {
            b.extend_from_slice(&((v.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
        }
    }
    for x in pose.local.iter().take(POS_BONES) {
        for v in [x.pos.x, x.pos.y, x.pos.z] {
            b.extend_from_slice(&v.to_le_bytes());
        }
    }
    if let Some(c) = cam {
        for v in [c.translation.x, c.translation.y, c.translation.z, c.rotation.x, c.rotation.y, c.rotation.z, c.rotation.w] {
            b.extend_from_slice(&v.to_le_bytes());
        }
    }
    b
}

type State = (Vec3, Quat, Vec<Quat>, Vec<Vec3>);

fn decode(b: &[u8]) -> Option<(State, Option<(Vec3, Quat)>)> {
    if b.len() < 35 || &b[0..4] != MAGIC || b[4] != VERSION {
        return None;
    }
    let f = |at: usize| b.get(at..at + 4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]));
    let pos = Vec3::new(f(5)?, f(9)?, f(13)?);
    let rot = Quat::from_xyzw(f(17)?, f(21)?, f(25)?, f(29)?).normalize();
    let n = u16::from_le_bytes([b[33], b[34]]) as usize;
    let mut at = 35;
    let mut rots = Vec::with_capacity(n);
    for _ in 0..n {
        let s = b.get(at..at + 8)?;
        let q = |k: usize| i16::from_le_bytes([s[k], s[k + 1]]) as f32 / 32767.0;
        rots.push(Quat::from_xyzw(q(0), q(2), q(4), q(6)).normalize());
        at += 8;
    }
    let mut poss = vec![];
    for _ in 0..n.min(POS_BONES) {
        poss.push(Vec3::new(f(at)?, f(at + 4)?, f(at + 8)?));
        at += 12;
    }
    let cam = (|| Some((Vec3::new(f(at)?, f(at + 4)?, f(at + 8)?), Quat::from_xyzw(f(at + 12)?, f(at + 16)?, f(at + 20)?, f(at + 24)?).normalize())))();
    pos.is_finite().then_some(((pos, rot, rots, poss), cam.filter(|c| c.0.is_finite())))
}

/// Send our player's state; take in the friend's.
pub fn sync(
    time: Res<Time>,
    mut net: ResMut<Net>,
    players: Query<(&Transform, &Character), With<Player>>,
    cams: Query<&Transform, (With<Camera3d>, Without<Character>)>,
) {
    let net = &mut *net;
    net.heard += time.delta_secs();
    net.up += time.delta_secs();
    let mut buf = [0u8; 4096];
    loop {
        match net.sock.recv_from(&mut buf) {
            Ok((n, from)) => {
                // (Hosting, the first sender is the friend; after that only they are listened to, while they send.)
                if net.hosting && Some(from) != net.peer && (net.peer.is_none() || net.heard > PEER_TIMEOUT) && decode(&buf[..n]).is_some() {
                    info!("net: {from} joined");
                    net.peer = Some(from);
                }
                if Some(from) != net.peer {
                    continue;
                }
                if let Some((s, cam)) = decode(&buf[..n]) {
                    if net.heard > HEARD {
                        info!("net: hearing {from}");
                    }
                    net.last = Some(s);
                    net.cam = cam;
                    net.heard = 0.0;
                    net.error = None;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            // (Windows reports the friend's closed port on the next receive: kept for the status line.)
            Err(e) => {
                net.error = Some(if e.kind() == std::io::ErrorKind::ConnectionReset { "nobody is listening there".into() } else { e.to_string() });
                break;
            }
        }
    }
    net.since_send += time.delta_secs();
    if net.since_send < SEND_EVERY {
        return;
    }
    net.since_send = 0.0;
    if let (Some(peer), Ok((tf, ch))) = (net.peer, players.single()) {
        let _ = net.sock.send_to(&encode(tf, &ch.pose, cams.single().ok()), peer);
    }
}

/// The friend's model on the animated floor: their figure is spawned without an animator, so with the procedural
/// rig's floor (its toes' height) under the model, but the poses heard are animated ones, made on floor 0. (It stood
/// in the air on each side, while each player saw themselves on the ground.)
pub fn ground_remote(remotes: Query<(), With<Remote>>, mut models: Query<(&ChildOf, &mut Transform), With<crate::character::ModelRoot>>) {
    for (parent, mut tf) in &mut models {
        if remotes.contains(parent.parent()) && tf.translation != Vec3::ZERO {
            tf.translation = Vec3::ZERO;
        }
    }
}

/// Pose the friend's figure from what was heard (after `character::animate`, which poses it as a statue).
pub fn apply(
    net: Res<Net>,
    mut remotes: Query<(&mut Transform, &mut Character, &mut Visibility, &mut Remote)>,
    mut joints: Query<&mut Transform, Without<Character>>,
) {
    for (mut tf, mut ch, mut vis, mut remote) in &mut remotes {
        let Some((pos, rot, rots, poss)) = net.last.as_ref().filter(|_| net.heard < HEARD) else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Inherited;
        // (Smoothed between packets.)
        tf.translation = tf.translation.lerp(*pos, 0.5);
        tf.rotation = tf.rotation.slerp(*rot, 0.5);
        let ch = &mut *ch;
        let mut pose = remote.shown.take().unwrap_or_else(|| ch.pose.clone());
        if pose.local.len() != rots.len() {
            continue;
        }
        for (i, x) in pose.local.iter_mut().enumerate() {
            x.rot = x.rot.slerp(rots[i], 0.6);
            if let Some(p) = poss.get(i) {
                x.pos = x.pos.lerp(*p, 0.6);
            }
        }
        ch.write_joints(&pose, &mut joints);
        ch.pose = pose.clone();
        remote.shown = Some(pose);
    }
}

/// A session started from the menu: make the friend's figure (hidden until they are heard) if there is none yet,
/// loading Altaïr's model again (a moment's pause, once).
#[allow(clippy::too_many_arguments)]
pub fn spawn_friend(
    mut commands: Commands,
    dir: Res<crate::GameDir>,
    mut tried: Local<bool>,
    remotes: Query<(), With<Remote>>,
    players: Query<&Transform, With<Player>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut bindposes: ResMut<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
) {
    if !remotes.is_empty() || std::mem::replace(&mut *tried, true) {
        return;
    }
    let model = match crate::assets::load_character(&dir.0, "DataPC.forge", "Rank 9", "UCMA_Altair_Rank_9") {
        Ok(m) => m,
        Err(e) => {
            warn!("net: no model for the friend: {e:#}");
            return;
        }
    };
    let at = players.single().map_or(Vec3::ZERO, |t| t.translation);
    let e = crate::character::spawn_character(&mut commands, model, at, &mut meshes, &mut mats, &mut images, &mut bindposes, None, 0);
    commands.entity(e).insert((crate::character::Statue, Remote::default(), Visibility::Hidden));
}

/// No session (left from the menu): the friend's figure hides.
pub fn hide_friend(mut remotes: Query<&mut Visibility, With<Remote>>) {
    for mut vis in &mut remotes {
        vis.set_if_neq(Visibility::Hidden);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_host_and_a_friend_hear_each_other_on_this_machine() {
        let mut host = host(0).unwrap();
        let mut friend = join(&format!("127.0.0.1:{}", host.port)).unwrap();
        assert!(host.status().contains("waiting"), "{}", host.status());
        let pose = ik::Pose { local: vec![] };
        let msg = encode(&Transform::from_xyz(1.0, 2.0, 3.0), &pose, None);
        friend.sock.send_to(&msg, friend.peer.unwrap()).unwrap();
        let mut buf = [0u8; 512];
        let mut got = None;
        for _ in 0..200 {
            if let Ok((n, from)) = host.sock.recv_from(&mut buf) {
                got = Some((decode(&buf[..n]).unwrap().0.0, from));
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let (pos, from) = got.expect("the host heard nothing");
        assert_eq!(pos, Vec3::new(1.0, 2.0, 3.0));
        (host.peer, host.heard) = (Some(from), 0.0);
        assert_eq!(host.peers(), 1);
        assert!(host.status().contains("connected"), "{}", host.status());
        friend.heard = 0.0;
        assert!(friend.status().starts_with("connected to 127.0.0.1"), "{}", friend.status());
    }
}
