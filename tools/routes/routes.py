"""The AC1 route library: short routes around the Damascus bureau, played in the real game (through ac1-hook) and in
ac1-rs with the same inputs, recorded frame by frame, compared.

routes.py real [name...]      record the real game's runs (AC1 in front, in Damascus, ac1-hook loaded)
routes.py ours [name...]      play them in ours (headless, in control mode, ROUTES_JOBS games at once), then `report`
routes.py report [name...]    how far ours' runs end and go from the real game's (both sides recorded)
routes.py side <name> [step]  the two runs side by side every `step` seconds (0.2)
routes.py diff <a> <b>        between two saved reports (`routes.py ours > a`), the routes that moved
routes.py list

A route: a start (game space x, y, z, yaw: degrees from +X toward +Y, the way he faces), how long, and its inputs, each
held over a span of seconds from the start: fwd (W), high (right mouse), legs (Space), back / left / right (S / A / D),
and taps (Space pressed then). Ours is given the keys at the frames the hook logged them and steered the way the real
game ran (its camera drifts), from the trace.

The traces are game data: they go to `lib/` here (git-ignored), or ROUTES_LIB (a path with no spaces: a control line
splits on whitespace). Ours' build is ROUTES_BIN (default target/agent/release/ac1.exe), the install AC1_GAME_DIR.
"""
import json, math, os, re, sys, time

S = os.path.dirname(os.path.abspath(__file__))
# compare.py's helpers (cmd, fps, where, stand, the action names, the open game's ours_run), not its main.
exec(open(os.path.join(S, "compare.py")).read().rsplit("\nmain()", 1)[0])

FREE = lambda secs: {"fwd": (0, secs), "high": (0, secs), "legs": (0, secs)}
ROOF1 = (53.0, 20.4, 11.96)   # the bureau's roof (bridge north, hops west and south, the long jump east)
ROOF2 = (41.0, 20.0, 11.85)   # the open roof west of it
STREET = (46.39, 6.13, 4.14)  # where the save starts, in the street by the bureau
WALL1 = (47.6, 20.0, 4.05)    # 1.4 m off the climbable wall by the bureau, facing it (+X)
WALL0 = (47.63, 15.5, 4.05)   # 2 m off the 4 m wall with the overhang, facing it (-Y)
SEAMS = (52.0, 33.4, 11.95)   # the roofs south of the hiding spot, with seams

ROUTES = {}


def route(name, at, yaw, secs, **inputs):
    ROUTES[name] = {"at": at, "yaw": yaw, "secs": secs, "inputs": inputs}


for yaw, way in [(0, "east"), (90, "north"), (180, "west"), (-90, "south")]:
    route(f"roof1_free_{way}", ROOF1, yaw, 5, **FREE(5))
    route(f"roof2_free_{way}", ROOF2, yaw, 4, **FREE(4))
    route(f"street_free_{way}", STREET, yaw, 4, **FREE(4))
# (North from the roof's middle, x 53, the way grazes the hiding spot's corner (x 53.1): a few degrees of camera decide
# whether he dives in, catches on the corner or runs past. Clear of it on the west, and straight at it.)
# (Nor any route north off the bureau's roof: each ended at the hiding box north of it, caught on it or hanging off its
# side (the user's photos). The run north goes too; the open roof west of it instead.)
for n in ["roof1_free_north", "roof1_run_north"]:
    ROUTES.pop(n, None)
# (The street's runs north and south only bounce off its walls, as wall0's does; and roof 2 west ran roof 1 west's line, the
# same ledge at the end: on a line of its own, 3.5 m north.)
for n in ["street_free_north", "street_free_south", "roof2_free_west"]:
    ROUTES.pop(n, None)
route("roof2_free_west", (41.0, 23.5, 11.85), 180, 4, **FREE(4))
route("roof2_walk_north", ROOF2, 90, 4, fwd=(0, 4))
route("roof2_run_east", ROOF2, 0, 4, fwd=(0, 4), high=(0, 4))
route("roof2_run_south", ROOF2, -90, 4, fwd=(0, 4), high=(0, 4))
route("roof1_walk_west", ROOF1, 180, 4, fwd=(0, 4))
route("roof1_walk_south", ROOF1, -90, 4, fwd=(0, 4))
route("roof1_run_west", ROOF1, 180, 4, fwd=(0, 4), high=(0, 4))
route("seams_free_west", SEAMS, 180, 5, **FREE(5))
route("seams_free_east", SEAMS, 0, 5, **FREE(5))
route("wall0_free", WALL0, -90, 4, **FREE(4))
route("wall1_free", WALL1, 0, 3, **FREE(3))
route("wall1_climb", WALL1, 0, 8, **FREE(8))
# Ejects during a wall run: the legs let go after it starts and pressed again on the way up (AC1 ejects on a fresh press,
# 0xE37590); W into the wall is straight back, A or D along the stick (`HumanWalling__ReboundJump`).
# The haystack on the roof north-east of the seams start (centre 54.32, 37.22, top 12.84).
route("hay70_free", SEAMS, 59, 4, **FREE(4))
route("hay70_walk", SEAMS, 59, 6, fwd=(0, 6))
# Ladders by the bureau: 68 up the bureau's west side from the street (faces west), 67 south-east (faces north).
route("ladder68_free", (33.6, 16.82, 3.98), 0, 7, **FREE(7))
route("ladder68_walk", (33.6, 16.82, 3.98), 0, 8, fwd=(0, 8))
# Diagonals and new ways from the same starts (do the rules hold where they were not tuned).
route("roof2_free_ne", ROOF2, 45, 4, **FREE(4))
route("roof2_free_nw", ROOF2, 135, 4, **FREE(4))
route("roof1_free_sw", ROOF1, -135, 5, **FREE(5))
route("seams_free_north", SEAMS, 90, 4, **FREE(4))
route("seams_free_south", SEAMS, -90, 4, **FREE(4))
route("street_run_east", STREET, 0, 4, fwd=(0, 4), high=(0, 4))
# Climbing from the wall run's hang: along it to either side, and up it without the legs (the plain moves).
route("wall1_hang_right", WALL1, 0, 5, fwd=(0, 1.0), legs=(0, 1.0), high=(0, 5), right=(1.6, 5))
route("wall1_hang_left", WALL1, 0, 5, fwd=(0, 1.0), legs=(0, 1.0), high=(0, 5), left=(1.6, 5))
route("wall1_hang_up", WALL1, 0, 6, fwd=(0, 6), legs=(0, 1.0), high=(0, 6))
# (Space let go 0.3 s in, before the wall: AC1's tap jump, at the wall's hold 1.3 m up.)
route("wall1_tapjump", WALL1, 0, 4, fwd=(0, 1.2), high=(0, 4), legs=(0, 0.3))
route("wall1_runeject_back", WALL1, 0, 4, fwd=(0, 1.2), high=(0, 4), legs=(0, 0.5), taps=[0.66])
route("wall1_runeject_left", WALL1, 0, 4, fwd=(0, 0.5), left=(0.5, 1.2), high=(0, 4), legs=(0, 0.5), taps=[0.66], _veer=True)
route("wall1_runeject_right", WALL1, 0, 4, fwd=(0, 0.5), right=(0.5, 1.2), high=(0, 4), legs=(0, 0.5), taps=[0.66], _veer=True)
# From the wall hang (the wall run onto it, the keys let go), the ejects: high profile, the legs tapped at 2.6 s.
for d in ["back", "left", "right", "none"]:
    extra = {} if d == "none" else {d: (2.4, 3.6)}
    route(f"wall1_eject_{d}", WALL1, 0, 5, fwd=(0, 1.0), legs=(0, 1.0), high=(0, 5), taps=[2.6], **extra)

# (live-compare, 2026-10-12) Standing jumps under the walls (AC1's `jumpstraight_footl_to_<kind>_<h>cm`, and LEDGE-7's
# `hangwallfree_250cm`), and the user's run onto the crate by the street start into the wall run and climb.
route("wall1_standjump", (48.75, 20.0, 4.05), 0, 3, high=(0, 3), legs=(0.2, 0.6))
route("wall0_standjump", (47.63, 14.0, 4.05), -90, 3, high=(0, 3), legs=(0.2, 0.6))
route("crate_climb", (45.916, 5.714, 4.137), -2, 6, fwd=(0, 4.7), high=(0, 4.26), legs=(0.14, 3.65))

KEY = {"fwd": "11", "back": "1f", "left": "1e", "right": "20", "legs": "39"}
# The camera is behind him when W walks him within this of the route's way (degrees): AC1 drifts with whatever is left.
ALIGN_DEG = 2.0
# Mouse counts per degree of camera turn (a first guess; each correction measures it again).
LOOK_PER_DEG = [6.0]


def align(x, y, z, yaw, rate):
    """Turn the camera until W walks him toward `yaw` (within `ALIGN_DEG`); he is put back at the start after."""
    prev = None
    for _ in range(10):
        cmd(f"tp {x} {y} {z} {yaw}", 1.0)
        # (Walked a second, measured over its second half: the first steps curve round to the camera's way.)
        cmd(f"key 11 {int(rate * 0.9)}", 0.4)
        a = where()
        time.sleep(0.5)
        b = where()
        if not a or not b:
            continue
        dx, dy = b[0] - a[0], b[1] - a[1]
        if math.hypot(dx, dy) < 0.1:
            # (Nowhere to walk that way: a quarter turn and try again.)
            off = 90.0
        else:
            off = (math.degrees(math.atan2(dy, dx)) - yaw + 180) % 360 - 180
        print(f"      camera: W goes {off:+.0f} degrees off", flush=True)
        if abs(off) < ALIGN_DEG:
            cmd(f"tp {x} {y} {z} {yaw}", 1.2)
            return True
        if prev is not None and abs(prev[0] - off) > 4 and prev[1] != 0:
            k = prev[1] / (prev[0] - off)
            if 0.5 < k < 100:
                LOOK_PER_DEG[0] = k
        counts = off * LOOK_PER_DEG[0]
        frames = 12
        cmd(f"look {round(counts / frames)} 0 {frames}", 0.8)
        prev = (off, round(counts / frames) * frames)
    return False


def real(name):
    r = ROUTES[name]
    x, y, z = r["at"]
    rate = fps()
    # At the start, standing, before anything is recorded (else skipped: a run from elsewhere is worse than none).
    for attempt in range(3):
        stand(rate)
        cmd(f"tp {x} {y} {z} {r['yaw']}", 1.5)
        w = where()
        if w and w[4] == "HumanGround" and math.hypot(w[0] - x, w[1] - y) < 0.5:
            break
        print(f"   (not at the start after the teleport: {w}; again)")
    else:
        print(f"   !! {name}: could not put him at the start: skipped")
        return
    # The camera his way: W runs the way the camera looks, and C after a teleport centres it on the facing he had
    # before (routes ran 48 or 90 degrees off). A short step in low profile shows where W goes; the mouse turns the
    # camera by the error; again until within 6 degrees.
    if not align(x, y, z, r["yaw"], rate):
        print(f"   !! {name}: the camera would not come round: skipped")
        return
    out = os.path.join(LIB, f"{name}.real.tsv").replace("\\", "/")
    secs = r["secs"]
    # Every input's start and end as a timeline of hook commands; the camera held centred throughout.
    events = [(0.0, f"trace {out}"), (0.0, f"key 2e {int(secs * rate)}")]
    for k, span in r["inputs"].items():
        # (Settings for ours only.)
        if k.startswith("_"):
            continue
        if k == "taps":
            events += [(t, f"key 39 {max(2, rate // 8)}") for t in span]
            continue
        a, b = span
        n = int((b - a) * rate)
        events.append((a, f"mouse 1 {n}" if k == "high" else f"key {KEY[k]} {n}"))
    events.sort(key=lambda e: e[0])
    t0 = time.time()
    i = 0
    while i < len(events):
        at = events[i][0]
        batch = [e[1] for e in events if e[0] == at]
        while time.time() - t0 < at:
            time.sleep(0.005)
        with open(os.path.join(G, "ac1-hook.cmd"), "w") as f:
            f.write("\n".join(batch) + "\n")
        time.sleep(0.12)  # the hook reads the file every 6 frames
        i += len(batch)
    while time.time() - t0 < secs + 1.5:
        time.sleep(0.05)
    cmd("trace off", 1.0)
    rows = [l.rstrip("\n").split("\t") for l in open(out)][1:]
    end = rows[-1] if rows else None
    # (The first metre the way he was set facing, else the camera was off: recorded again.)
    for c in rows:
        dx, dy = float(c[2]) - float(rows[0][2]), float(c[3]) - float(rows[0][3])
        if math.hypot(dx, dy) > 1.0:
            off = (math.degrees(math.atan2(dy, dx)) - r["yaw"] + 180) % 360 - 180
            if abs(off) > 20:
                print(f"   !! {name}: ran {off:+.0f} degrees off the facing (the camera): again")
                os.remove(out)
                return False
            break
    acts = [ids.get(int(a, 16)) or keys.get((rr[6], int(a, 16))) or a for rr in rows if len(rr) > 7 and rr[7] for a in rr[7].split(",")]
    acts = [a for a in acts if "wait" not in a and "lookaround" not in a]
    print(f"   {name}: {len(rows)} frames, ends {end[6] if end else '?'} at ({float(end[2]):.2f}, {float(end[3]):.2f}, {float(end[4]):.2f})"
          f", {len(acts)} moves: {', '.join(a.replace('xx_', '') for a in acts[:8])}{' ...' if len(acts) > 8 else ''}" if end else f"   {name}: no frames")


def real_heading(r, name):
    """The way the real game ran over its first 1.5 m (degrees), when it did: W follows the camera, set within 6 degrees of
    the route's way, and those degrees grow to metres over a route; ours starts the way the real one went."""
    path = os.path.join(LIB, f"{name}.real.tsv")
    if not name or not os.path.exists(path):
        return r["yaw"]
    # (Between 2 and 6 m out, on the ground: over its first steps the real one turns to the camera's way.)
    x, y, _ = r["at"]
    a = b = None
    for l in open(path):
        p = l.rstrip("\n").split("\t")
        if len(p) < 7 or p[0] == "frame":
            continue
        if p[6] != "HumanGround":
            if a:
                break
            continue
        q = (float(p[2]), float(p[3]))
        d = math.hypot(q[0] - x, q[1] - y)
        if d >= 2.0 and a is None:
            a = q
        if a and d <= 6.0:
            b = q
        if d > 6.0:
            break
    if not a or not b or math.hypot(b[0] - a[0], b[1] - a[1]) < 1.0:
        return r["yaw"]
    got = math.degrees(math.atan2(b[1] - a[1], b[0] - a[0]))
    off = (got - r["yaw"] + 180) % 360 - 180
    return r["yaw"] + off if abs(off) < 15 else r["yaw"]


def real_headings(name):
    """The way the real game moved on the ground over time, every 0.2 s while it ran (`AC1_HEADINGS`): W follows its
    camera, which turns slowly as it runs; ours is steered the same way."""
    path = os.path.join(LIB, f"{name}.real.tsv")
    if not name or not os.path.exists(path):
        return []
    rows = []
    for l in open(path):
        p = l.rstrip("\n").split("\t")
        if len(p) >= 7 and p[0] != "frame":
            rows.append((float(p[1]), float(p[2]), float(p[3]), p[6], float(p[5])))
    out, next_t = [], 0.0
    for i, (t, x, y, ctx, yaw) in enumerate(rows):
        if t < next_t or ctx != "HumanGround":
            continue
        # (Over the next 0.2 s, on the ground and moving at 1 m/s or more.)
        j = next((k for k in range(i, len(rows)) if rows[k][0] >= t + 0.2), None)
        if j is None or any(rows[k][3] != "HumanGround" for k in range(i, j + 1)):
            continue
        dx, dy = rows[j][1] - x, rows[j][2] - y
        if math.hypot(dx, dy) / (rows[j][0] - t) < 1.0:
            continue
        # (Only going the way he faces: sliding along a wall the way moved is not the stick's.)
        if abs((math.degrees(math.atan2(dy, dx)) - yaw + 180) % 360 - 180) > 25:
            continue
        # (Off the ground between, the last heading held up to the next: not blended across a climb or a jump.)
        if out and t + 0.1 - out[-1][0] > 0.4:
            out.append((t + 0.05, out[-1][1]))
        out.append((t + 0.1, math.degrees(math.atan2(dy, dx))))
        next_t = t + 0.2
    return out


def real_keys(name):
    """When the real game's recording actually pressed each key (the hook logs the frame of each command; Python's timing
    drifts by frames): {key hex or "m1": [(from, to) seconds, ...]} on the trace's clock."""
    path = os.path.join(LIB, f"{name}.real.tsv")
    if not name or not os.path.exists(path):
        return {}
    rows = [l.split("	") for l in open(path).read().splitlines()[1:]]
    frames = [(int(c[0]), float(c[1])) for c in rows if len(c) > 1]
    if len(frames) < 2:
        return {}
    (f0, t0), (f1, t1) = frames[0], frames[-1]
    sec = lambda f: t0 + (f - f0) * (t1 - t0) / max(1, f1 - f0)
    log = open(os.path.join(G, "ac1-hook.log"), errors="replace").read().splitlines()
    mark = f"{name}.real.tsv"
    # (The entry this trace was written by: its frame just before the trace's first; the hook's frames restart with the game.)
    def at(l):
        m = re.match(r"\[frame (\d+)\]", l)
        return int(m.group(1)) if m else -1
    start = max((i for i, l in enumerate(log) if "trace into" in l and l.endswith(mark) and 0 <= f0 - at(l) < 30), default=None)
    if start is None:
        return {}
    out = {}
    for l in log[start + 1:]:
        if "trace off" in l or "trace into" in l:
            break
        m = re.match(r"\[frame (\d+)\] (key ([0-9a-f]+) for|mouse button 1) (\d+)", l)
        if m:
            k = m.group(3) or "m1"
            f = int(m.group(1))
            out.setdefault(k, []).append((max(0.0, round(sec(f), 3)), max(0.0, round(sec(f + int(m.group(4))), 3))))
    return out


def as_pressed(i, name):
    """The route's inputs at the times the real game got them (`real_keys`), where it logged them."""
    keys = real_keys(name)
    if not keys:
        return i
    i = dict(i)
    for k, hexk in [("fwd", "11"), ("back", "1f"), ("left", "1e"), ("right", "20"), ("high", "m1")]:
        if k in i and keys.get(hexk):
            i[k] = keys[hexk][0]
    presses = keys.get("39", [])
    if "legs" in i:
        held = [p for p in presses if p[1] - p[0] > 0.25 or abs(p[0] - i["legs"][0]) < 0.05]
        if held:
            i["legs"] = held[0]
    if "taps" in i:
        taps = [p[0] for p in presses if p[1] - p[0] <= 0.25 and not (abs(p[0] - i.get("legs", (9, 9))[0]) < 0.05)]
        if len(taps) == len(i["taps"]):
            i["taps"] = taps
    return i


def ours_settings(r, trace, name=""):
    """The route's inputs as ours' script settings (AC1_WALK, AC1_HIGH, ..., AC1_CLIMB)."""
    i, secs = as_pressed(r["inputs"], name), r["secs"]
    x, y, z = r["at"]
    # (From where the real run's recording starts: the camera's align step before it can leave him a little on, 0.4 m on
    # `wall1_hang_up`, and ours then ran 0.15 s behind all the way.)
    try:
        with open(os.path.join(LIB, f"{name}.real.tsv")) as f:
            first = next(l for l in f if l[:1].isdigit()).split("	")
        fx, fy, fz = float(first[2]), float(first[3]), float(first[4])
        if math.hypot(fx - x, fy - y) < 1.0:
            x, y = fx, fy
    except (OSError, StopIteration, IndexError, ValueError):
        pass
    yaw = real_heading(r, name)
    st = {"AC1_START": f"{x},{-y},{yaw - 90:.2f},{z + 1.5}", "AC1_TRACE": trace}
    covers = lambda k, span: k in i and i[k][0] <= span[0] and i[k][1] >= span[1]
    if "fwd" in i:
        f = i["fwd"]
        st["AC1_WALK"] = "6.2" if covers("high", f) and covers("legs", f) else ("5.2" if covers("high", f) else "1.5")
        # (The stick held right over, the speed from ours' gait model as the player's: AC1's walk is 1.9 m/s, not the
        # fixed 1.5, and its high-profile jog 3.5, not 5.2.)
        if os.environ.get("ROUTES_FIXED_SPEED") is None:
            st["AC1_STICK"] = "1"
        st["AC1_STOP"] = f"{f[1]}"
        hs = real_headings(name)
        if hs and os.environ.get("ROUTES_NO_HEADINGS") is None and "_veer" not in i:
            st["AC1_HEADINGS"] = ";".join(f"{t:.2f},{d:.1f}" for t, d in hs)
    if "high" in i:
        st["AC1_HIGH"] = f"{i['high'][0]}-{i['high'][1]}"
    if "legs" in i:
        st["AC1_LEGS"] = f"{i['legs'][0]}-{i['legs'][1]}"
    taps = list(i.get("taps", []))
    # (Space held 0.35 s or less and let go: AC1's tap jump, on the release, 0xEE817E; ours jumps on a press.)
    pass  # (the tap: AC1_LEGS does the release itself now)
    if taps:
        st["AC1_JUMP"] = ",".join(str(t) for t in sorted(taps))
    # On a wall: W is up (as the real game climbs with it), S / A / D down, left, right; in time order.
    steps = []
    if "fwd" in i:
        steps.append((i["fwd"][0], i["fwd"][1], "up"))
    for k, d in [("back", "down"), ("left", "left"), ("right", "right")]:
        if k in i:
            steps.append((i[k][0], i[k][1], d))
    # (A side key off the ground, during a wall run: ours' stick swung that way (`AC1_VEER`), held to its end; on the wall
    # the same key as a climbing direction.)
    if "_veer" in i:
        k = "left" if "left" in i else "right"
        st["AC1_VEER"] = f"{i[k][0]},{90 if k == 'left' else -90}"
        st["AC1_STOP"] = f"{i[k][1]}"
    steps.sort()
    t, climb = 0.0, []
    for a, b, d in steps:
        if a > t:
            climb.append(f"wait={a - t:.2f}")
        # (The legs held over it: on a wall they leap, as the real game's climbing jumps.)
        leap = "leap-" if covers("legs", (max(a, t), b)) else ""
        climb.append(f"{leap}{d}={b - max(a, t):.2f}")
        t = max(t, b)
    if climb:
        st["AC1_CLIMB"] = "nograb," + ",".join(climb)
    return st


def ours(name):
    r = ROUTES[name]
    out = os.path.join(LIB, f"{name}.ours.tsv")
    ours_run(name, r["secs"], ours_settings(r, out, name))
    return out


# AC1's contexts for ours' climber states.
CONTEXT = [("ground", "Ground"), ("ground_act", "Ground"), ("land", "Ground"), ("top", "Ground"), ("lean", "Ground"),
           ("jump", "InAir"), ("fall", "InAir"), ("vault", "InAir"), ("leap", "InAir"), ("drop", "InAir"), ("dead", "Dead"),
           ("wallrun", "Walling"), ("hangwall", "Ledge"), ("hangwall_open", "Ledge"), ("free", "Ledge"), ("hangknee", "Ledge"),
           ("kneel", "Ledge"), ("perch", "NarrowObject"), ("hay", "HayStack"), ("hayout", "HayStack"), ("swing", "Swing")]


def ctx_of(state):
    for k, v in CONTEXT:
        if state == k:
            return v
    if state[:1].isdigit():
        return "Climb"
    return state


def load(path, real):
    rows = []
    for l in open(path).read().splitlines()[1:]:
        c = l.split("\t")
        if real:
            if len(c) >= 7:
                rows.append((float(c[1]), float(c[2]), float(c[3]), float(c[4]), c[6].replace("Human", "")))
        elif len(c) >= 6:
            rows.append((float(c[0]), float(c[1]), float(c[2]), float(c[3]), ctx_of(c[5])))
    return rows


def contexts(rows):
    out = []
    for r in rows:
        if not out or out[-1] != r[4]:
            out.append(r[4])
    return out


def compare(name):
    for side in ["real", "ours"]:
        if not os.path.exists(os.path.join(LIB, f"{name}.{side}.tsv")):
            print(f"   ({name}: no {side} run yet)")
            return None
    a = load(os.path.join(LIB, f"{name}.real.tsv"), True)
    b = load(os.path.join(LIB, f"{name}.ours.tsv"), False)
    if not a or not b:
        return None
    at = lambda rows, t: min(rows, key=lambda r: abs(r[0] - t))
    secs = ROUTES[name]["secs"]
    gaps = [math.dist(at(a, t)[1:4], at(b, t)[1:4]) for t in [k * 0.1 for k in range(int(secs * 10) + 1)]]
    end = math.dist(at(a, secs)[1:4], at(b, secs)[1:4])
    # (Where each comes to rest: the last rows.)
    final = math.dist(a[-1][1:4], b[-1][1:4])
    return {"end": end, "final": final, "mean": sum(gaps) / len(gaps), "worst": max(gaps), "real": contexts(a), "ours": contexts(b),
            "real_end": at(a, secs)[1:4], "ours_end": at(b, secs)[1:4]}


def report(names):
    res = [(n, compare(n)) for n in names]
    res = [(n, c) for n, c in res if c]
    res.sort(key=lambda nc: -nc[1]["end"])
    print(f"{'route':22} {'end':>6} {'final':>6} {'mean':>6} {'worst':>6}  states (real | ours)")
    for n, c in res:
        print(f"{n:22} {c['end']:6.2f} {c['final']:6.2f} {c['mean']:6.2f} {c['worst']:6.2f}  {' > '.join(c['real'])}  |  {' > '.join(c['ours'])}")
    ok = sum(1 for _, c in res if c["end"] < 1.0)
    rest = sum(1 for _, c in res if c["final"] < 1.0)
    print(f"\n{ok} of {len(res)} routes end within 1 m of the real game's ({rest} come to rest within 1 m); mean path gap {sum(c['mean'] for _, c in res) / max(1, len(res)):.2f} m")


def main():
    if len(sys.argv) < 2 or sys.argv[1] == "list":
        for n, r in ROUTES.items():
            print(n, r)
        return
    names = sys.argv[2:] or list(ROUTES)
    if sys.argv[1] == "real":
        json.dump(ROUTES, open(os.path.join(LIB, "routes.json"), "w"), indent=1)
        for n in names:
            for _ in range(3):
                if real(n) is not False:
                    break
    elif sys.argv[1] == "ours":
        # (In parallel: ROUTES_JOBS games at once, each its own worker with its own game; the longest routes dealt first.)
        jobs = int(os.environ.get("ROUTES_JOBS", "4"))
        if jobs > 1 and len(names) > 1:
            names_by = sorted(names, key=lambda n: -ROUTES[n]["secs"])
            parts = [names_by[k::jobs] for k in range(jobs)]
            procs = []
            for k, part in enumerate(parts):
                if part:
                    env = dict(os.environ, ROUTES_WORKER=str(k + 1))
                    procs.append(subprocess.Popen([sys.executable, os.path.abspath(__file__), "ours-part", *part], env=env))
            for p in procs:
                p.wait()
        else:
            try:
                for n in names:
                    ours(n)
            finally:
                ours_quit()
        report(names)
    elif sys.argv[1] == "ours-part":
        try:
            for n in names:
                ours(n)
        finally:
            ours_quit()
    elif sys.argv[1] == "report":
        report(names)
    elif sys.argv[1] == "side":
        side(sys.argv[2], float(sys.argv[3]) if len(sys.argv) > 3 else 0.2)
    elif sys.argv[1] == "diff":
        diff(sys.argv[2], sys.argv[3])


def side(name, step):
    """Both runs every `step` seconds: position (game space), yaw, context; ours' clip where one starts."""
    a = [r for r in (l.split("\t") for l in open(os.path.join(LIB, f"{name}.real.tsv")).read().splitlines()[1:]) if len(r) >= 7]
    b = [r for r in (l.split("\t") for l in open(os.path.join(LIB, f"{name}.ours.tsv")).read().splitlines()[1:]) if len(r) >= 6]
    at = lambda rows, k, t: min(rows, key=lambda r: abs(float(r[k]) - t))
    for i in range(int(ROUTES[name]["secs"] / step) + 3):
        t = i * step
        p, q = at(a, 1, t), at(b, 0, t)
        clip = next((r[6] for r in b if len(r) > 6 and r[6] and t - step / 2 <= float(r[0]) < t + step / 2), "")
        print(f"{t:4.1f}  real {float(p[2]):7.2f} {float(p[3]):7.2f} {float(p[4]):6.2f} {float(p[5]):5.0f} {p[6].replace('Human', ''):13} | ours "
              f"{float(q[1]):7.2f} {float(q[2]):7.2f} {float(q[3]):6.2f} {float(q[4]):5.0f} {q[5]:10} {clip}")


def diff(pa, pb):
    """The routes whose end or mean gap moved by more than 0.1 / 0.05 m between two saved reports."""
    def load_report(p):
        out = {}
        for l in open(p).read().splitlines():
            c = l.split()
            if len(c) > 5 and c[1].replace(".", "").isdigit():
                out[c[0]] = (float(c[1]), float(c[3]))
        return out
    a, b = load_report(pa), load_report(pb)
    for k in sorted(a, key=lambda k: b.get(k, a[k])[0] - a[k][0]):
        if k in b and (abs(b[k][0] - a[k][0]) > 0.1 or abs(b[k][1] - a[k][1]) > 0.05):
            print(f"{k:22} end {a[k][0]:5.2f} -> {b[k][0]:5.2f}   mean {a[k][1]:5.2f} -> {b[k][1]:5.2f}")


main()
