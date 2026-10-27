"""Run one move test in the real AC1 (through ac1-hook) and in ac1-rs, and print both move sequences side by side.

compare.py <name> <x> <y> <z> <yaw> <secs> <inputs>
compare.py --ours <same>      (ours only)
compare.py --real <same>      (the real game only)
  x y z: game space (Z up), yaw: degrees, the way he faces (from +X toward +Y)
  inputs: comma list of fwd, high, legs (held from the start for `secs`); climb: on a wall, W is up (ours' script)
The real game must be in front (fullscreen it takes no keys otherwise); dllcheck.py checks the hook first.
"""
import math, os, re, subprocess, sys, time

S = os.path.dirname(os.path.abspath(__file__))
# The install (with ac1-hook in it), the repo, ours' build, and where runs and traces go: game data, never committed
# (`lib/` here is git-ignored). ROUTES_LIB must have no spaces in its path: a control line splits on whitespace.
G = os.environ.get("AC1_GAME_DIR", "P:/SteamLibrary/steamapps/common/Assassins Creed")
REPO = os.path.abspath(os.path.join(S, "..", ".."))
BIN = os.environ.get("ROUTES_BIN") or os.path.join(REPO, "target", "agent", "release", "ac1.exe")
LIB = os.environ.get("ROUTES_LIB") or os.path.join(S, "lib")
os.makedirs(LIB, exist_ok=True)
LOG = os.path.join(G, "ac1-hook.log")

# AC1's action names by id (the hook's table, `livelog names` writes it into the install).
ids, keys = {}, {}
for l in open(os.path.join(G, "ac1-hook.actions.tsv")):
    p = l.rstrip("\n").split("\t")
    if len(p) == 4:
        ids[int(p[0], 16)] = p[3]
        keys.setdefault((p[2], int(p[1], 16)), p[3])


def cmd(text, wait=1.5):
    with open(os.path.join(G, "ac1-hook.cmd"), "w") as f:
        f.write(text + "\n")
    time.sleep(wait)


def log_lines():
    return open(LOG, errors="replace").read().splitlines()


def fps():
    """The game's frame rate now, from the hook's frame counter (key holds are counted in frames)."""
    st = os.path.join(G, "ac1-hook.status")
    a = int(open(st).read().split()[1])
    time.sleep(1.0)
    b = int(open(st).read().split()[1])
    return max(30, b - a)


def where():
    """(x, y, z, yaw, context) now, from a fresh answer to the hook's `where`."""
    n = len(log_lines())
    cmd("where", 0.3)
    for _ in range(30):
        said = [l for l in log_lines()[n:] if "player at" in l]
        if said:
            m = re.search(r"player at \(([-\d.]+), ([-\d.]+), ([-\d.]+)\) yaw ([-\d.]+) in (\w+)", said[-1])
            if m:
                return (*map(float, m.groups()[:4]), m.group(5))
        time.sleep(0.2)
    return None


def stand(rate):
    """Get him standing (teleported in a hang or crouched on a beam, he stays in that state: the user saw him lie on a
    roof in the hang's pose until Shift got him out)."""
    for k in range(8):
        w = where()
        if w and w[4] == "HumanGround":
            return True
        ctx = w[4] if w else "?"
        # A hang: E (empty hand) lets go. On a beam or in hay: W, then W with the legs (off the end, a jump off it). Else
        # Shift, back.
        # (E does not let go of a hang; the back eject, high profile + the legs + back, does.)
        if ctx in ("HumanLedge", "HumanClimb", "HumanLadder"):
            # (Every other try Shift: teleported in a hang, he hangs on at the new spot, and only Shift ends that.)
            keys_ = ["m1", "39", "1f"] if k % 2 == 0 else ["2a"]
        elif ctx in ("HumanNarrowObject", "HumanHayStack"):
            # (A teleport takes him off a beam onto the ground; the jump off it found a ledge and looped.)
            cmd("tp 46.39 6.13 4.14 0", 2.0)
            continue
        else:
            keys_ = [["2a"], ["1f"]][k % 2]
        print(f"   (real AC1 in {ctx}: keys {keys_} to get him out)")
        cmd("\n".join(f"mouse 1 {rate // 2}" if c == "m1" else f"key {c} {rate // 2}" for c in keys_), 3.0)
    print("   (real AC1 still not standing)")
    return False


def real(name, x, y, z, yaw, secs, inputs):
    out = os.path.join(LIB, f"cmp_{name}.tsv").replace("\\", "/")
    rate = fps()
    stand(rate)
    cmd(f"tp {x} {y} {z} {yaw}", 1.2)
    w = where()
    if not w or math.hypot(w[0] - x, w[1] - y) > 0.3:
        print(f"   !! the teleport did not take: {w}")
    # C (the right stick's click): the camera behind him, held a while and given time to swing round (W runs the way
    # the camera looks).
    cmd(f"key 2e {rate // 2}", 2.5)
    frames = int(secs * rate)
    # (C held through the run too: the camera stays behind him, so W keeps to the facing.)
    lines = [f"trace {out}", f"key 2e {frames}"]
    if "high" in inputs:
        lines.append(f"mouse 1 {frames}")
    if "legs" in inputs:
        lines.append(f"key 39 {frames}")
    if "fwd" in inputs:
        lines.append(f"key 11 {frames}")
    cmd("\n".join(lines), secs + 2.0)
    cmd("trace off", 1.0)
    rows = [l.rstrip("\n").split("\t") for l in open(out)][1:]
    # (W follows the camera: the first metre should go the way he was set facing.)
    for r in rows:
        if len(r) >= 5 and math.hypot(float(r[2]) - x, float(r[3]) - y) > 1.0:
            got = math.degrees(math.atan2(float(r[3]) - y, float(r[2]) - x))
            off = (got - yaw + 180) % 360 - 180
            if abs(off) > 30:
                print(f"   !! real AC1 ran {off:+.0f} degrees off the facing (the camera was not behind him): not comparable")
            break
    seq = []
    for r in rows:
        if len(r) < 8:
            continue
        for a in filter(None, r[7].split(",")):
            v = int(a, 16)
            clip = ids.get(v) or keys.get((r[6], v)) or f"{v:#x}"
            seq.append((float(r[1]), r[6].replace("Human", ""), clip, (float(r[2]), float(r[3]), float(r[4]))))
    if rows:
        r = rows[-1]
        seq.append((float(r[1]), r[6].replace("Human", ""), "(end)", (float(r[2]), float(r[3]), float(r[4]))))
    return seq


# Ours stays open in control mode (AC1_CONTROL): Damascus loads once, each run is a line in its command file.
# (A worker of a parallel run has its own game and control files: ROUTES_WORKER=k.)
WORKER = os.environ.get("ROUTES_WORKER", "")
CTL_CMD = os.path.join(LIB, f"ours_ctl{WORKER}.cmd")
CTL_LOG = os.path.join(LIB, f"ours_ctl{WORKER}.log")
PROC = None
# (Settings for the game itself, not a run's.)
OURS_ONCE = {"AC1_LEVEL", "AC1_SHOT", "AC1_SHOT_SECS", "AC1_NO_CROWD", "AC1_BACKGROUND", "AC1_CONTROL"}


def ours_alive():
    # (Only the game this script started: another ac1.exe may be the user's.)
    return PROC is not None and PROC.poll() is None and os.path.exists(CTL_LOG)


def ours_quit():
    """Ends the game this script started (a control file's `quit`), so none is left running."""
    if PROC is not None and PROC.poll() is None:
        with open(CTL_CMD, "w") as f:
            f.write("quit\n")
        try:
            PROC.wait(30)
        except subprocess.TimeoutExpired:
            PROC.kill()


def ours_start():
    """Ours in control mode, in the background (the level loads once, about a minute)."""
    for f in (CTL_CMD, CTL_LOG):
        if os.path.exists(f):
            os.remove(f)
    env = dict(os.environ)
    env.update({"AC1_LEVEL": "damascus", "AC1_NO_CROWD": "1", "AC1_BACKGROUND": "1", "AC1_CONTROL": CTL_CMD,
                "AC1_HEADLESS": "1", "AC1_STEP": "60", "AC1_MUTE": "1",
                "RUST_LOG": "info,ac1::climb=debug", "NO_COLOR": "1"})
    global PROC
    PROC = subprocess.Popen([BIN], cwd=REPO, env=env, stdout=open(CTL_LOG, "wb"), stderr=subprocess.STDOUT)
    for _ in range(600):
        if "ready in" in open(CTL_LOG, encoding="utf-8", errors="replace").read():
            time.sleep(2)
            return
        time.sleep(0.5)


def ours_run(name, secs, settings):
    """One run in the open game: its log lines, from started to done."""
    if not ours_alive():
        ours_start()
    rid = f"{name}-{int(time.time() * 1000)}"
    with open(CTL_CMD, "w") as f:
        f.write(f"run {rid} {secs + 1} " + " ".join(f"{k}={v}" for k, v in settings.items()) + "\n")
    lines = []
    for _ in range(int((secs + 300) * 4)):
        text = open(CTL_LOG, encoding="utf-8", errors="replace").read().splitlines(True)
        try:
            i = next(k for k, l in enumerate(text) if f"control: run {rid} started" in l)
        except StopIteration:
            time.sleep(0.25)
            continue
        done = [k for k, l in enumerate(text[i:]) if f"control: run {rid} done" in l]
        if done:
            lines = text[i:i + done[0] + 1]
            break
        time.sleep(0.25)
    # (The end, as the one-shot runs logged it.)
    return [l.replace(f"control: run {rid} done, at", "script: the run ends at") for l in lines]


def ours(name, x, y, z, yaw, secs, inputs):
    env = dict(os.environ)
    speed = "6.2" if {"high", "legs"} <= set(inputs) else ("5.2" if "high" in inputs else "1.5")
    env.update({
        "AC1_LEVEL": "damascus", "AC1_SHOT": "-", "AC1_SHOT_SECS": str(secs + 1), "AC1_NO_CROWD": "1",
        "AC1_START": f"{x},{-y},{yaw - 90},{z + 1.5}", "RUST_LOG": "info,ac1::climb=debug", "NO_COLOR": "1",
        "AC1_BACKGROUND": "1",
    })
    if "fwd" in inputs:
        env["AC1_WALK"] = speed
        env["AC1_STOP"] = str(secs)  # (let go when the real game's keys are)
        if "climb" in inputs:
            # (W up the wall; with Space held the real game climbs, it does not leap: up, not leap-up.)
            env["AC1_CLIMB"] = f"nograb,up={secs}"
    if "high" in inputs:
        env["AC1_HIGH"] = f"0-{secs}"
    if "legs" in inputs:
        env["AC1_LEGS"] = f"0-{secs}"
    log = os.path.join(LIB, f"ours_{name}.log")
    with open(log, "w", encoding="utf-8") as f:
        f.writelines(ours_run(name, secs, {k: v for k, v in env.items() if k.startswith("AC1_") and k not in OURS_ONCE}))
    seq = []
    for l in open(log, encoding="utf-8", errors="replace"):
        m = re.search(r"climb: (\w+) -> (\w+) via (\S+) at \[([-\d.]+), ([-\d.]+), ([-\d.]+)\]", l)
        if m:
            bx, by, bz = map(float, m.groups()[3:])
            seq.append((m.group(1), m.group(2), f"{m.group(3)}   at ({bx:.2f}, {-bz:.2f}, {by:.2f})"))
        elif ("landed from" in l or "jump aimed" in l or "rebound" in l) and "climb:" in l or "the run ends" in l:
            seq.append(("", "", l.split("climb: ")[-1].split("script: ")[-1].strip()))
    return seq


def main():
    mode = sys.argv[1] if sys.argv[1] in ("--ours", "--real") else ""
    a = sys.argv[2:] if mode else sys.argv[1:]
    name, x, y, z, yaw, secs, inputs = a[0], *map(float, a[1:6]), a[6].split(",")
    print(f"== {name}: game ({x}, {y}, {z}) yaw {yaw}, {secs} s, {inputs}", flush=True)
    # (Ours runs in the background while the real game does its run: about a minute of loading saved per test.)
    import threading
    got = {}
    worker = None
    if mode != "--real":
        worker = threading.Thread(target=lambda: got.update(seq=ours(name, x, y, z, yaw, secs, inputs)))
        worker.start()
    if mode != "--ours":
        print("-- real AC1:")
        for t, ctx, clip, p in real(name, x, y, z, yaw, secs, inputs):
            print(f"   {t:6.2f}s {ctx:12} {clip}   at ({p[0]:.2f}, {p[1]:.2f}, {p[2]:.2f})", flush=True)
    if worker:
        worker.join()
        print("-- ac1-rs:")
        for f, t, clip in got.get("seq", []):
            print(f"   {f:>12} -> {t:12} {clip}")
    ours_quit()


main()
