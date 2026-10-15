//! Online play over UDP, up to `MAX_PLAYERS` together: the online menu (F10: hosting on a port, or joining an address)
//! or, for test runs, `AC1_HOST=port` hosts and `AC1_JOIN=ip:port` joins. The host is the hub: each friend sends only to
//! it, it gives each a player id (1 up; the host is 0) and passes every packet on to the others, so everyone sees
//! everyone, each other player an Altaïr posed from them. A friend quiet `PEER_TIMEOUT` gives their place up. Each side
//! sends its player's root and pose about 30 times a second. No game data crosses the wire, only transforms.
//!
//! Packet: "AC1N", version, the sender's player id (the host's 0; a friend's own packets carry `UNASSIGNED`, the host
//! writes their id in when passing them on), root position (3 x f32), root rotation (4 x f32), bone count (u16), then
//! each bone's rotation as 4 x i16 (x 32767), the first `POS_BONES` bones' positions as 3 x f32 (the rest keep the bind
//! pose: both sides read the same skeleton), then the sender's camera, position and rotation (7 x f32: for replays, the
//! friend's point of view).
use bevy::prelude::*;
use std::net::{SocketAddr, UdpSocket};

use crate::character::{Character, Player};

const MAGIC: &[u8; 4] = b"AC1N";
const VERSION: u8 = 3;
/// Players together at most, the host included.
pub const MAX_PLAYERS: usize = 8;
/// The id a friend's own packets carry (the host knows them by their address).
const UNASSIGNED: u8 = 255;
/// Bones whose positions are sent too (the root and hips move; the rest are fixed lengths).
const POS_BONES: usize = 4;
/// Seconds between sends.
const SEND_EVERY: f32 = 1.0 / 30.0;
/// A player is shown while heard within this (s).
const HEARD: f32 = 3.0;
/// Hosting, a friend quiet this long (s) gives their place up.
const PEER_TIMEOUT: f32 = 6.0;

type State = (Vec3, Quat, Vec<Quat>, Vec<Vec3>);
/// A packet read: the sender's id, their state and their camera.
type Packet = (u8, State, Option<(Vec3, Quat)>);

/// One other player's last state: root and bone transforms, their camera, and how long ago (s).
struct Heard {
    state: State,
    cam: Option<(Vec3, Quat)>,
    ago: f32,
}

#[derive(Resource)]
pub struct Net {
    sock: UdpSocket,
    /// Joining: the host we talk to.
    peer: Option<SocketAddr>,
    hosting: bool,
    /// Hosting: the friends' addresses, their player ids and how long each has been quiet (s).
    friends: Vec<(SocketAddr, u8, f32)>,
    since_send: f32,
    /// Every other player's state last heard, by player id.
    players: std::collections::BTreeMap<u8, Heard>,
    /// Seconds since anything was heard.
    heard: f32,
    /// Seconds since the socket opened.
    up: f32,
    /// Our port.
    port: u16,
    /// What went wrong last on receiving (Windows: nobody listening at the address joined).
    error: Option<String>,
}

/// Another player's figure: their player id, and the pose last shown on it (smoothed toward what is heard).
#[derive(Component, Default)]
pub struct Remote {
    id: u8,
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

/// Host on UDP `port`: friends join it.
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
        Ok(Net { sock, peer, hosting, friends: vec![], since_send: SEND_EVERY, players: Default::default(), heard: f32::MAX, up: 0.0, port, error: None })
    }

    pub fn hosting(&self) -> bool {
        self.hosting
    }

    /// Our UDP port.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Players heard now, besides us.
    pub fn peers(&self) -> usize {
        self.players.values().filter(|h| h.ago < HEARD).count()
    }

    /// One line for the menu and the log: listening, connected, how many, what went wrong.
    pub fn status(&self) -> String {
        let peers = self.peers();
        let s = if self.hosting {
            if self.friends.is_empty() {
                format!("hosting: listening on UDP port {}, waiting for friends (up to {})", self.port, MAX_PLAYERS - 1)
            } else {
                format!("hosting on UDP port {}: {} of {} friends connected", self.port, self.friends.len(), MAX_PLAYERS - 1)
            }
        } else {
            match self.peer {
                Some(p) if self.heard < HEARD => format!("connected to {p}"),
                Some(p) if self.heard < f32::MAX => format!("joined {p}: no packets for {:.0} s", self.heard),
                Some(p) => format!("joining {p}: no answer yet ({:.0} s){}", self.up, self.error.as_ref().map_or(String::new(), |e| format!(": {e}"))),
                None => "not connected".to_string(),
            }
        };
        format!("{s} ({peers} other player{})", if peers == 1 { "" } else { "s" })
    }

    /// A friend's camera (the first heard), for replays.
    pub fn friend_camera(&self) -> Option<(Vec3, Quat)> {
        self.players.values().find(|h| h.ago < HEARD).and_then(|h| h.cam)
    }
}

fn encode(id: u8, root: &Transform, pose: &ik::Pose, cam: Option<&Transform>) -> Vec<u8> {
    let mut b = Vec::with_capacity(41 + pose.local.len() * 8 + POS_BONES * 12);
    b.extend_from_slice(MAGIC);
    b.push(VERSION);
    b.push(id);
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

/// The byte holding a packet's sender id.
const ID_AT: usize = 5;

/// A packet's sender id, its state and its camera.
fn decode(b: &[u8]) -> Option<Packet> {
    if b.len() < 36 || &b[0..4] != MAGIC || b[4] != VERSION {
        return None;
    }
    let id = b[ID_AT];
    let f = |at: usize| b.get(at..at + 4).map(|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]));
    let pos = Vec3::new(f(6)?, f(10)?, f(14)?);
    let rot = Quat::from_xyzw(f(18)?, f(22)?, f(26)?, f(30)?).normalize();
    let n = u16::from_le_bytes([b[34], b[35]]) as usize;
    let mut at = 36;
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
    pos.is_finite().then_some((id, (pos, rot, rots, poss), cam.filter(|c| c.0.is_finite())))
}

impl Net {
    /// Take in one packet from `from` (`buf` its bytes). Hosting, a new address takes the first free id while there is
    /// room, and the packet goes on to every other friend with the sender's id written in.
    fn take(&mut self, buf: &mut [u8], from: SocketAddr) {
        let Some((id, state, cam)) = decode(buf) else { return };
        let id = if self.hosting {
            let k = match self.friends.iter().position(|f| f.0 == from) {
                Some(k) => k,
                None if self.friends.len() + 1 < MAX_PLAYERS => {
                    let free = (1..MAX_PLAYERS as u8).find(|i| self.friends.iter().all(|f| f.1 != *i)).unwrap_or(1);
                    info!("net: {from} joined as player {free}");
                    self.friends.push((from, free, 0.0));
                    self.friends.len() - 1
                }
                None => return,
            };
            self.friends[k].2 = 0.0;
            let id = self.friends[k].1;
            buf[ID_AT] = id;
            for f in self.friends.iter().filter(|f| f.0 != from) {
                let _ = self.sock.send_to(buf, f.0);
            }
            id
        } else if Some(from) == self.peer {
            id
        } else {
            return;
        };
        if self.players.get(&id).is_none_or(|h| h.ago > HEARD) {
            info!("net: hearing player {id}");
        }
        self.players.insert(id, Heard { state, cam, ago: 0.0 });
        self.heard = 0.0;
        self.error = None;
    }

    /// Time passes: hosting, a friend quiet too long gives their place up (their figure hides with them).
    fn tick(&mut self, dt: f32) {
        self.heard += dt;
        self.up += dt;
        for h in self.players.values_mut() {
            h.ago += dt;
        }
        for f in &mut self.friends {
            f.2 += dt;
        }
        if self.hosting {
            for f in self.friends.iter().filter(|f| f.2 > PEER_TIMEOUT) {
                info!("net: player {} left", f.1);
                self.players.remove(&f.1);
            }
            self.friends.retain(|f| f.2 <= PEER_TIMEOUT);
        }
    }

    /// Send our state: hosting to every friend, joining to the host.
    fn send(&self, root: &Transform, pose: &ik::Pose, cam: Option<&Transform>) {
        if self.hosting {
            let msg = encode(0, root, pose, cam);
            for f in &self.friends {
                let _ = self.sock.send_to(&msg, f.0);
            }
        } else if let Some(peer) = self.peer {
            let _ = self.sock.send_to(&encode(UNASSIGNED, root, pose, cam), peer);
        }
    }
}

/// Send our player's state; take in everyone else's (hosting, passing each friend's on to the others).
pub fn sync(
    time: Res<Time>,
    mut net: ResMut<Net>,
    players: Query<(&Transform, &Character), With<Player>>,
    cams: Query<&Transform, (With<Camera3d>, Without<Character>)>,
) {
    let net = &mut *net;
    net.tick(time.delta_secs());
    let mut buf = [0u8; 4096];
    loop {
        match net.sock.recv_from(&mut buf) {
            Ok((n, from)) => net.take(&mut buf[..n], from),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            // (Windows reports a closed port on the next receive: hosting, a friend who left; joining, kept for the
            // status line.)
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset && net.hosting => continue,
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
    if let Ok((tf, ch)) = players.single() {
        net.send(tf, &ch.pose, cams.single().ok());
    }
}

/// The other players' models on the animated floor: their figures are spawned without an animator, so with the
/// procedural rig's floor (its toes' height) under the model, but the poses heard are animated ones, made on floor 0.
/// (They stood in the air on each side, while each player saw themselves on the ground.)
pub fn ground_remote(remotes: Query<(), With<Remote>>, mut models: Query<(&ChildOf, &mut Transform), With<crate::character::ModelRoot>>) {
    for (parent, mut tf) in &mut models {
        if remotes.contains(parent.parent()) && tf.translation != Vec3::ZERO {
            tf.translation = Vec3::ZERO;
        }
    }
}

/// Pose each other player's figure from what was heard (after `character::animate`, which poses it as a statue).
pub fn apply(
    net: Res<Net>,
    mut remotes: Query<(&mut Transform, &mut Character, &mut Visibility, &mut Remote)>,
    mut joints: Query<&mut Transform, Without<Character>>,
) {
    for (mut tf, mut ch, mut vis, mut remote) in &mut remotes {
        let Some((pos, rot, rots, poss)) = net.players.get(&remote.id).filter(|h| h.ago < HEARD).map(|h| &h.state) else {
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

/// A figure (hidden until heard) for each other player heard who has none yet; Altaïr's model is loaded once and kept.
#[allow(clippy::too_many_arguments)]
pub fn spawn_friend(
    mut commands: Commands,
    dir: Res<crate::GameDir>,
    net: Res<Net>,
    mut model: Local<Option<Option<crate::assets::CharacterData>>>,
    remotes: Query<&Remote>,
    players: Query<&Transform, With<Player>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut bindposes: ResMut<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
) {
    let missing: Vec<u8> = net.players.keys().copied().filter(|id| remotes.iter().all(|r| r.id != *id)).collect();
    if missing.is_empty() {
        return;
    }
    let model = model.get_or_insert_with(|| match crate::assets::load_character(&dir.0, "DataPC.forge", "Rank 9", "UCMA_Altair_Rank_9") {
        Ok(m) => Some(m),
        Err(e) => {
            warn!("net: no model for the other players: {e:#}");
            None
        }
    });
    let Some(model) = model.as_ref() else { return };
    let at = players.single().map_or(Vec3::ZERO, |t| t.translation);
    for id in missing {
        let e = crate::character::spawn_character(&mut commands, model.clone(), at, &mut meshes, &mut mats, &mut images, &mut bindposes, None, 0);
        commands.entity(e).insert((crate::character::Statue, Remote { id, shown: None }, Visibility::Hidden));
    }
}

/// No session (left from the menu): the other players' figures hide.
pub fn hide_friend(mut remotes: Query<&mut Visibility, With<Remote>>) {
    for mut vis in &mut remotes {
        vis.set_if_neq(Visibility::Hidden);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read every packet waiting on `n`'s socket into it (a short while, for the loopback).
    fn pump(n: &mut Net) {
        let mut buf = [0u8; 4096];
        for _ in 0..40 {
            while let Ok((len, from)) = n.sock.recv_from(&mut buf) {
                n.take(&mut buf[..len], from);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn three_players_see_each_other_through_the_host() {
        let mut host = host(0).unwrap();
        let at = format!("127.0.0.1:{}", host.port);
        let (mut a, mut b) = (join(&at).unwrap(), join(&at).unwrap());
        assert!(host.status().contains("waiting"), "{}", host.status());
        let pose = ik::Pose { local: vec![] };
        a.send(&Transform::from_xyz(1.0, 0.0, 0.0), &pose, None);
        pump(&mut host);
        b.send(&Transform::from_xyz(2.0, 0.0, 0.0), &pose, None);
        pump(&mut host);
        // (Each sends all the time: the next packet of A's, now B is there, goes on to B.)
        a.send(&Transform::from_xyz(1.0, 0.0, 0.0), &pose, None);
        pump(&mut host);
        host.send(&Transform::from_xyz(3.0, 0.0, 0.0), &pose, None);
        pump(&mut a);
        pump(&mut b);
        // (The host hears both friends; each friend hears the host and the other friend, passed on.)
        assert_eq!(host.peers(), 2, "{}", host.status());
        assert!(host.status().contains("2 of 7 friends"), "{}", host.status());
        let at_of = |n: &Net, id: u8| n.players.get(&id).map(|h| h.state.0.x);
        assert_eq!((at_of(&host, 1), at_of(&host, 2)), (Some(1.0), Some(2.0)));
        assert_eq!((at_of(&a, 0), at_of(&a, 2)), (Some(3.0), Some(2.0)));
        assert_eq!((at_of(&b, 0), at_of(&b, 1)), (Some(3.0), Some(1.0)));
        assert!(a.status().starts_with("connected to 127.0.0.1"), "{}", a.status());
    }

    #[test]
    fn a_quiet_friend_gives_their_place_up() {
        let mut host = host(0).unwrap();
        let a = join(&format!("127.0.0.1:{}", host.port)).unwrap();
        a.send(&Transform::IDENTITY, &ik::Pose { local: vec![] }, None);
        pump(&mut host);
        assert_eq!(host.friends.len(), 1);
        host.tick(PEER_TIMEOUT + 1.0);
        assert!(host.friends.is_empty() && host.players.is_empty());
    }
}
