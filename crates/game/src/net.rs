//! A quick two-player test over UDP (branch `feature/multiplayer`, not merged into dev or main yet): `AC1_HOST=port` waits for a friend,
//! `AC1_JOIN=ip:port` connects to one. Each side sends its player's root and pose about 30 times a second; the other
//! player shows as a second Altaïr posed from them. No game data crosses the wire, only transforms.
//!
//! Packet: "AC1N", version, root position (3 x f32), root rotation (4 x f32), bone count (u16), then each bone's
//! rotation as 4 x i16 (x 32767), and the first `POS_BONES` bones' positions as 3 x f32 (the rest keep the bind pose:
//! both sides read the same skeleton).
use bevy::prelude::*;
use std::net::{SocketAddr, UdpSocket};

use crate::character::{Character, Player};

const MAGIC: &[u8; 4] = b"AC1N";
const VERSION: u8 = 1;
/// Bones whose positions are sent too (the root and hips move; the rest are fixed lengths).
const POS_BONES: usize = 4;
/// Seconds between sends.
const SEND_EVERY: f32 = 1.0 / 30.0;

#[derive(Resource)]
pub struct Net {
    sock: UdpSocket,
    /// Who we talk to: given when joining, learned from the first packet when hosting.
    peer: Option<SocketAddr>,
    hosting: bool,
    since_send: f32,
    /// The last state heard: root and bone transforms.
    last: Option<(Vec3, Quat, Vec<Quat>, Vec<Vec3>)>,
    heard: f32,
}

/// The other player's figure, and the pose last shown on it (smoothed toward what is heard).
#[derive(Component, Default)]
pub struct Remote {
    shown: Option<ik::Pose>,
}

/// Is a network session asked for (`AC1_HOST` or `AC1_JOIN`)?
pub fn enabled() -> bool {
    std::env::var("AC1_HOST").is_ok() || std::env::var("AC1_JOIN").is_ok()
}

/// Open the socket: host on `AC1_HOST`'s port, or join `AC1_JOIN`. (Before the app's logging starts: printed.)
pub fn open() -> Option<Net> {
    let (sock, peer, hosting) = if let Ok(port) = std::env::var("AC1_HOST") {
        let port: u16 = port.trim().parse().unwrap_or(7777);
        let s = UdpSocket::bind(("0.0.0.0", port)).map_err(|e| eprintln!("net: cannot listen on UDP {port}: {e}")).ok()?;
        eprintln!("net: hosting on UDP port {port}, waiting for a friend");
        (s, None, true)
    } else {
        let to: SocketAddr = std::env::var("AC1_JOIN").ok()?.trim().parse().map_err(|e| eprintln!("net: AC1_JOIN wants ip:port ({e})")).ok()?;
        let s = UdpSocket::bind(("0.0.0.0", 0)).map_err(|e| eprintln!("net: cannot open a UDP socket: {e}")).ok()?;
        eprintln!("net: joining {to}");
        (s, Some(to), false)
    };
    sock.set_nonblocking(true).ok()?;
    Some(Net { sock, peer, hosting, since_send: SEND_EVERY, last: None, heard: f32::MAX })
}

fn encode(root: &Transform, pose: &ik::Pose) -> Vec<u8> {
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
    b
}

fn decode(b: &[u8]) -> Option<(Vec3, Quat, Vec<Quat>, Vec<Vec3>)> {
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
    pos.is_finite().then_some((pos, rot, rots, poss))
}

/// Send our player's state; take in the friend's.
pub fn sync(time: Res<Time>, mut net: ResMut<Net>, players: Query<(&Transform, &Character), With<Player>>) {
    let net = &mut *net;
    net.heard += time.delta_secs();
    let mut buf = [0u8; 4096];
    loop {
        match net.sock.recv_from(&mut buf) {
            Ok((n, from)) => {
                // (Hosting, the first sender is the friend; after that only they are listened to.)
                if net.hosting && net.peer.is_none() {
                    info!("net: {from} joined");
                    net.peer = Some(from);
                }
                if Some(from) != net.peer {
                    continue;
                }
                if let Some(s) = decode(&buf[..n]) {
                    if net.heard > 3.0 {
                        info!("net: hearing {from}");
                    }
                    net.last = Some(s);
                    net.heard = 0.0;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            // (Windows reports the friend's closed port on the next receive: ignore it.)
            Err(_) => break,
        }
    }
    net.since_send += time.delta_secs();
    if net.since_send < SEND_EVERY {
        return;
    }
    net.since_send = 0.0;
    if let (Some(peer), Ok((tf, ch))) = (net.peer, players.single()) {
        let _ = net.sock.send_to(&encode(tf, &ch.pose), peer);
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
        let Some((pos, rot, rots, poss)) = net.last.as_ref().filter(|_| net.heard < 3.0) else {
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
