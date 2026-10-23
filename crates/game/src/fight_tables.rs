//! AC1's fight action tables, as the executable fills them at start (AssassinsCreed_Dx9.exe v1.02: the initializers
//! 0xF319A0, 0xF324E0, 0xF32E00, ... and the record setters 0xF314C0 / 0xF315F0, read in Ghidra). Numbers only: action
//! ids of `Human_ActionKit`'s fight blocks (`actions <game> <block>` names their clips), `NONE` where the exe leaves the
//! slot empty. Weapons in the exe's order: 0 fists, 1 sword (`light`), 2 short blade (`knife`); the turns have a 4th,
//! the hidden blade (`blade`). Feet: 0 left, 1 right. Directions: 0 ul, 1 dl, 2 dr, 3 ur. How they are picked is in
//! `fight_data` (and docs/COMBAT.md).

use crate::fight_data::Strike;

/// No action in this slot.
pub const NONE: u32 = u32::MAX;

/// Attack wind-ups [weapon][strong, max][side front/left/right/back][distance short/medium/long][foot] (0x1A31920,
/// 0x1A31A48; the attack start 0xF3E2F0).
pub const ATTACK_START: [[[[[u32; 2]; 3]; 4]; 2]; 3] = [
    [
        [
            [[0x1b4d5810, 0x1b4d580d], [0x1b4d5807, 0x1b4d580a], [0x1b4d5807, 0x1b4d580a]],
            [[0x1b4d5819, 0x1b4d581c], [0x1b4d5813, 0x1b4d5816], [0x1b4d5813, 0x1b4d5816]],
            [[0x1b4d5825, 0x1b4d5828], [0x1b4d581f, 0x1b4d5822], [0x1b4d581f, 0x1b4d5822]],
            [[0x1b4d5804, 0x1b4d5801], [0x1b4d57fb, 0x1b4d57fe], [0x1b4d57fb, 0x1b4d57fe]],
        ],
        [
            [[0x1b4d58d6, 0x1b4d58d3], [0x1b4d58d0, 0x1b4d58cd], [0x1b4d58d0, 0x1b4d58cd]],
            [[0x1b4d58df, 0x1b4d58e2], [0x1b4d58d9, 0x1b4d58dc], [0x1b4d58d9, 0x1b4d58dc]],
            [[0x1b4d5927, 0x1b4d592a], [0x1b4d5921, 0x1b4d5924], [0x1b4d5921, 0x1b4d5924]],
            [[0x1b4d58c7, 0x1b4d58ca], [0x1b4d58c4, 0x1b4d58c1], [0x1b4d58c4, 0x1b4d58c1]],
        ],
    ],
    [
        [
            [[0x11228474, 0x11228478], [0x1122846c, 0x11228470], [0x11228464, 0x11228468]],
            [[0x1122848c, 0x11228490], [0x11228484, 0x11228488], [0x1122847c, 0x11228480]],
            [[0x112284a4, 0x112284a8], [0x1122849c, 0x112284a0], [0x11228494, 0x11228498]],
            [[0x1122845c, 0x11228460], [0x11228454, 0x11228458], [0x1122844c, 0x11228450]],
        ],
        [
            [[0x120fe5a9, 0x11a7019d], [0x11a70195, 0x11a70199], [0x11a7018d, 0x11a70191]],
            [[0x120fe5ac, 0x11a701b1], [0x11a701a9, 0x11a701ad], [0x11a701a1, 0x11a701a5]],
            [[0x120fe5af, 0x11a701c5], [0x11a701bd, 0x11a701c1], [0x11a701b5, 0x11a701b9]],
            [[0x120fe5a6, 0x11a70189], [0x11a70181, 0x11a70185], [0x11a70179, 0x11a7017d]],
        ],
    ],
    [
        [
            [[0x2c168956, 0x2c168953], [0x2c168956, 0x2c168953], [0x2c168956, 0x2c168953]],
            [[0x2c168959, 0x2c16895c], [0x2c168959, 0x2c16895c], [0x2c168959, 0x2c16895c]],
            [[0x2c16895f, 0x2c168962], [0x2c16895f, 0x2c168962], [0x2c16895f, 0x2c168962]],
            [[0x2c168950, 0x2c16894d], [0x2c168950, 0x2c16894d], [0x2c168950, 0x2c16894d]],
        ],
        [
            [[0x2c168956, 0x2c168953], [0x2c168956, 0x2c168953], [0x2c168956, 0x2c168953]],
            [[0x2c168959, 0x2c16895c], [0x2c168959, 0x2c16895c], [0x2c168959, 0x2c16895c]],
            [[0x2c16895f, 0x2c168962], [0x2c16895f, 0x2c168962], [0x2c16895f, 0x2c168962]],
            [[0x2c168950, 0x2c16894d], [0x2c168950, 0x2c16894d], [0x2c168950, 0x2c16894d]],
        ],
    ],
];
/// The strikes after them, {action, the direction the blade ends in}: same layout (records at 0x1A33C18 / 0x1A33F78, set
/// by 0xF314C0 for the blades and 0xF315F0 for the fists, whose long distance repeats the medium one).
pub const ATTACK_STRIKE: [[[[[Strike; 2]; 3]; 4]; 2]; 3] = [
    [
        [
            [[(0x1b4d5811, 3), (0x1b4d580e, 0)], [(0x1b4d5808, 3), (0x1b4d580b, 0)], [(0x1b4d5808, 3), (0x1b4d580b, 0)]],
            [[(0x1b4d581a, 3), (0x1b4d581d, 3)], [(0x1b4d5814, 3), (0x1b4d5817, 3)], [(0x1b4d5814, 3), (0x1b4d5817, 3)]],
            [[(0x1b4d5826, 0), (0x1b4d5829, 0)], [(0x1b4d5820, 0), (0x1b4d5823, 3)], [(0x1b4d5820, 0), (0x1b4d5823, 3)]],
            [[(0x1b4d5805, 3), (0x1b4d5802, 0)], [(0x1b4d57fc, 0), (0x1b4d57ff, 3)], [(0x1b4d57fc, 0), (0x1b4d57ff, 3)]],
        ],
        [
            [[(0x1b4d58d7, 2), (0x1b4d58d4, 1)], [(0x1b4d58d1, 2), (0x1b4d58ce, 1)], [(0x1b4d58d1, 2), (0x1b4d58ce, 1)]],
            [[(0x1b4d58e0, 2), (0x1b4d58e3, 2)], [(0x1b4d58da, 2), (0x1b4d58dd, 2)], [(0x1b4d58da, 2), (0x1b4d58dd, 2)]],
            [[(0x1b4d5928, 1), (0x1b4d592b, 1)], [(0x1b4d5922, 1), (0x1b4d5925, 1)], [(0x1b4d5922, 1), (0x1b4d5925, 1)]],
            [[(0x1b4d58c8, 1), (0x1b4d58cb, 2)], [(0x1b4d58c5, 1), (0x1b4d58c2, 2)], [(0x1b4d58c5, 1), (0x1b4d58c2, 2)]],
        ],
    ],
    [
        [
            [[(0x11228516, 3), (0x11228517, 1)], [(0x11228514, 3), (0x11228515, 3)], [(0x11228512, 3), (0x11228513, 0)]],
            [[(0x1122851c, 3), (0x1122851d, 3)], [(0x1122851a, 2), (0x1122851b, 2)], [(0x11228518, 3), (0x11228519, 2)]],
            [[(0x11228523, 1), (0x11228522, 0)], [(0x11228520, 1), (0x11228521, 3)], [(0x1122851e, 0), (0x1122851f, 1)]],
            [[(0x11228510, 0), (0x11228511, 3)], [(0x1122850e, 0), (0x1122850f, 2)], [(0x1122850c, 0), (0x1122850d, 3)]],
        ],
        [
            [[(0x120fe5a8, 3), (0x11a701d0, 0)], [(0x11a701ce, 3), (0x11a701cf, 3)], [(0x11a701cc, 0), (0x11a701cd, 1)]],
            [[(0x120fe5ab, 0), (0x11a701d5, 2)], [(0x11a701d3, 2), (0x11a701d4, 3)], [(0x11a701d1, 2), (0x11a701d2, 3)]],
            [[(0x120fe5ae, 1), (0x11a701da, 3)], [(0x11a701d8, 1), (0x11a701d9, 3)], [(0x11a701d6, 1), (0x11a701d7, 0)]],
            [[(0x120fe5a5, 1), (0x11a701cb, 2)], [(0x11a701c9, 0), (0x11a701ca, 2)], [(0x11a701c7, 0), (0x11a701c8, 2)]],
        ],
    ],
    [
        [
            [[(0x2c168957, 2), (0x2c168954, 1)], [(0x2c168957, 2), (0x2c168954, 1)], [(0x2c168957, 2), (0x2c168954, 1)]],
            [[(0x2c16895a, 1), (0x2c16895d, 3)], [(0x2c16895a, 1), (0x2c16895d, 3)], [(0x2c16895a, 1), (0x2c16895d, 3)]],
            [[(0x2c168960, 2), (0x2c168963, 3)], [(0x2c168960, 2), (0x2c168963, 3)], [(0x2c168960, 2), (0x2c168963, 3)]],
            [[(0x2c16894e, 1), (0x2c168951, 3)], [(0x2c16894e, 1), (0x2c168951, 3)], [(0x2c16894e, 1), (0x2c168951, 3)]],
        ],
        [
            [[(0x2c168957, 2), (0x2c168954, 1)], [(0x2c168957, 2), (0x2c168954, 1)], [(0x2c168957, 2), (0x2c168954, 1)]],
            [[(0x2c16895a, 1), (0x2c16895d, 3)], [(0x2c16895a, 1), (0x2c16895d, 3)], [(0x2c16895a, 1), (0x2c16895d, 3)]],
            [[(0x2c168960, 2), (0x2c168963, 3)], [(0x2c168960, 2), (0x2c168963, 3)], [(0x2c168960, 2), (0x2c168963, 3)]],
            [[(0x2c16894e, 1), (0x2c168951, 3)], [(0x2c16894e, 1), (0x2c168951, 3)], [(0x2c16894e, 1), (0x2c168951, 3)]],
        ],
    ],
];
/// The sword's first attacks straight ahead at medium and long distance: [strong, max][foot][distance][blade direction],
/// the direction picked at random, not the last one (0x1A31D90 and the strikes 0x1A31FD8; 0xF3E2F0, the sword only).
pub const FRONT_START: [[[[u32; 4]; 3]; 2]; 2] = [
    [
        [[NONE, NONE, NONE, NONE], [0x27ce104e, 0x27ce1042, 0x27ce1048, 0x1122846c], [0xa5d05d78, 0xa5d05d6c, 0xa5d05d72, 0x11228464]],
        [[NONE, NONE, NONE, NONE], [0x27ce1051, 0x27ce1045, 0x27ce104b, 0x11228470], [0x11228468, 0xa5d05d6f, 0xa5d05d75, 0xa5d05d7b]],
    ],
    [
        [[NONE, NONE, NONE, NONE], [0x27ce1060, 0x27ce1054, 0x27ce105a, 0x11a70195], [0x11a7018d, 0xa5d05d9c, 0xa5d05d9f, 0xa5d05da5]],
        [[NONE, NONE, NONE, NONE], [0x27ce1063, 0x27ce1057, 0x27ce105d, 0x11a70199], [0xa5d05e1f, 0x11a70191, 0xa5d05da2, 0xa5d05da8]],
    ],
];
pub const FRONT_STRIKE: [[[[u32; 4]; 3]; 2]; 2] = [
    [
        [[NONE, NONE, NONE, NONE], [0x27ce104f, 0x27ce1043, 0x27ce1049, 0x11228514], [0xa5d05d79, 0xa5d05d6d, 0xa5d05d73, 0x11228512]],
        [[NONE, NONE, NONE, NONE], [0x27ce1052, 0x27ce1046, 0x27ce104c, 0x11228515], [0x11228513, 0xa5d05d70, 0xa5d05d76, 0xa5d05d7c]],
    ],
    [
        [[NONE, NONE, NONE, NONE], [0x27ce1061, 0x27ce1055, 0x27ce105b, 0x11a701ce], [0x11a701cc, 0xa5d05d9d, 0xa5d05da0, 0xa5d05da6]],
        [[NONE, NONE, NONE, NONE], [0x27ce1064, 0x27ce1058, 0x27ce105e, 0x11a701cf], [0xa5d05e20, 0x11a701cd, 0xa5d05da3, 0xa5d05da9]],
    ],
];
/// Combo wind-ups [weapon][direction][strong, max][foot] (0x1A31B70) and their strikes (records at 0x1A34638).
pub const COMBO_START: [[[[u32; 2]; 2]; 4]; 3] = [
    [[[NONE, NONE], [NONE, NONE]], [[NONE, NONE], [NONE, NONE]], [[NONE, NONE], [NONE, NONE]], [[NONE, NONE], [NONE, NONE]]],
    [
        [[0x1cd350cc, 0x1cd350cf], [0x27ce1494, 0x27ce1497]],
        [[0x1cd35058, 0x1cd3505b], [0x27ce1488, 0x27ce148b]],
        [[0x1cd3505e, 0x1cd35061], [0x27ce148e, 0x27ce1491]],
        [[0x1cd35177, 0x1cd3517a], [0x27ce149a, 0x27ce149d]],
    ],
    [
        [[NONE, NONE], [NONE, NONE]],
        [[0x2c16930e, 0x2c16930b], [NONE, NONE]],
        [[0x2c16930e, 0x2c169311], [NONE, NONE]],
        [[0x2c169314, 0x2c169317], [NONE, NONE]],
    ],
];
pub const COMBO_STRIKE: [[[[Strike; 2]; 2]; 4]; 3] = [
    [
        [[(NONE, 0), (NONE, 0)], [(NONE, 0), (NONE, 0)]],
        [[(NONE, 0), (NONE, 0)], [(NONE, 0), (NONE, 0)]],
        [[(NONE, 0), (NONE, 0)], [(NONE, 0), (NONE, 0)]],
        [[(NONE, 0), (NONE, 0)], [(NONE, 0), (NONE, 0)]],
    ],
    [
        [[(0x1cd350cd, 0), (0x1cd350d0, 0)], [(0x27ce1495, 0), (0x27ce1498, 0)]],
        [[(0x1cd35059, 1), (0x1cd3505c, 1)], [(0x27ce1489, 1), (0x27ce148c, 1)]],
        [[(0x1cd3505f, 2), (0x1cd35062, 2)], [(0x27ce148f, 2), (0x27ce1492, 2)]],
        [[(0x1cd35178, 3), (0x1cd3517b, 3)], [(0x27ce149b, 3), (0x27ce149e, 3)]],
    ],
    [
        [[(NONE, 0), (NONE, 0)], [(NONE, 0), (NONE, 0)]],
        [[(0x2c169309, 1), (0x2c16930c, 1)], [(NONE, 1), (NONE, 1)]],
        [[(0x2c16930f, 2), (0x2c169312, 2)], [(NONE, 2), (NONE, 2)]],
        [[(0x2c169315, 3), (0x2c169318, 3)], [(NONE, 3), (NONE, 3)]],
    ],
];
/// What the attacker plays when its strike lands: on the body (`hit`, 0x1A32398, played by 0xF4A0A0) and on a block
/// (`blocked`, 0x1A322D0, 0xF3B820): [weapon][the blade's direction][strong, max][foot].
pub const HIT_OUT: [[[[u32; 2]; 2]; 4]; 3] = [
    [[[NONE, NONE], [NONE, NONE]], [[NONE, NONE], [NONE, NONE]], [[NONE, NONE], [NONE, NONE]], [[NONE, NONE], [NONE, NONE]]],
    [
        [[0x01bb2ce5, 0x01bb2cf9], [0xcd651b90, 0xcd651b92]],
        [[0x0e46fc99, 0x0e46fc9a], [0xcd651b88, 0xcd651b8a]],
        [[0x0e46fc98, 0x01bb2cf7], [0xcd651b8c, 0xcd651b8e]],
        [[0x01bb2ce2, 0x01bb2cf5], [0xcd651b94, 0xcd651b96]],
    ],
    [[[NONE, NONE], [NONE, NONE]], [[NONE, 0x2c16a238], [NONE, NONE]], [[0x2c16a239, 0x2c16a23a], [NONE, NONE]], [[0x2c16a23b, 0x2c16a23c], [NONE, NONE]]],
];
pub const BLOCKED_OUT: [[[[u32; 2]; 2]; 4]; 3] = [
    [[[NONE, NONE], [NONE, NONE]], [[NONE, NONE], [NONE, NONE]], [[NONE, NONE], [NONE, NONE]], [[NONE, NONE], [NONE, NONE]]],
    [
        [[NONE, NONE], [0xecd1a7a5, 0xecd1a7a7]],
        [[0x0e46fc90, 0x01ca0907], [0x0e46fc90, 0xecd1a7a1]],
        [[0x0e46fc91, NONE], [0xecd1a7a3, NONE]],
        [[NONE, NONE], [0xecd1a7a9, 0xecd1a7ab]],
    ],
    [
        [[0x2c1699bb, 0x2c1699bd], [0x2c1699bb, 0x2c1699bd]],
        [[0x2c1699b5, 0x2c1699b5], [0x2c1699b5, 0x2c1699b5]],
        [[0x2c1699b7, 0x2c1699b9], [0x2c1699b7, 0x2c1699b9]],
        [[0x2c1699bb, 0x2c1699bd], [0x2c1699bb, 0x2c1699bd]],
    ],
];
/// The turns on the spot in the stance [weapon, 3 the hidden blade][45 left, 45 right, 90 left, 90 right, 180][defence,
/// stance][foot] (0x1A33AD0, played by 0xF38D10).
pub const TURN: [[[[u32; 2]; 2]; 5]; 4] = [
    [
        [[0x168fc4fd, 0x168fc4fe], [0x168fc4f3, 0x168fc4f4]],
        [[0x168fc4ff, 0x168fc500], [0x168fc4f5, 0x168fc4f6]],
        [[0x168fc501, 0x168fc502], [0x168fc4f7, 0x168fc4f8]],
        [[0x168fc503, 0x168fc504], [0x168fc4f9, 0x168fc4fa]],
        [[0x168fc505, 0x168fc506], [0x168fc4fb, 0x168fc4fc]],
    ],
    [
        [[0x168fc392, 0x168fc393], [0x14cdf75f, 0x14cdf760]],
        [[0x168fc394, 0x168fc395], [0x14cdf761, 0x14cdf762]],
        [[0x168fc396, 0x168fc397], [0x14cdf763, 0x14cdf764]],
        [[0x168fc398, 0x168fc399], [0x14cdf765, 0x14cdf766]],
        [[0x168fc39a, 0x168fc39b], [0x14cdf621, 0x14cdf622]],
    ],
    [
        [[0x2c16a230, 0x2c16a22e], [0x2c1688dd, 0x2c1688db]],
        [[0x2c16a231, 0x2c16a22f], [0x2c1688de, 0x2c1688dc]],
        [[0x2c16a233, 0x2c16a232], [0x2c1688e1, 0x2c1688df]],
        [[0x2c16a235, 0x2c16a234], [0x2c1688e2, 0x2c1688e0]],
        [[0x2c16a236, 0x2c16a237], [0x2c1688e3, 0x2c1688e4]],
    ],
    [
        [[0x69b54ba0, 0x69b54b9e], [0x69b54bb4, 0x69b54bb2]],
        [[0x69b54ba1, 0x69b54b9f], [0x69b54bb5, 0x69b54bb3]],
        [[0x69b54ba4, 0x69b54ba2], [0x69b54bb8, 0x69b54bb6]],
        [[0x69b54ba5, 0x69b54ba3], [0x69b54bb9, 0x69b54bb7]],
        [[0x69b54b9c, 0x69b54b9d], [0x69b54bb0, 0x69b54bb1]],
    ],
];
/// The paired combo kill [weapon][the combo hit's direction]: the killer's wind-up, step in and kill (0x1A33538,
/// 0x1A3356C, 0x1A335A0), the victim's death pose and death, by 3 - the direction (0x1A335D4, 0x1A33608).
pub const CK_START: [[u32; 4]; 3] =
    [[NONE, NONE, NONE, NONE], [0xa5d058fb, 0xa5d05907, 0xa5d05901, 0x4a60ce93], [0xa5d059fb, 0xa5d059ef, 0xa5d059f5, 0xa5d059fb]];
pub const CK_POSITION: [[u32; 4]; 3] =
    [[NONE, NONE, NONE, NONE], [0xa5d058fa, 0xa5d05906, 0xa5d05900, 0x4a60ce94], [0xa5d059fc, 0xa5d059f0, 0xa5d059f6, 0xa5d059fc]];
pub const CK_KILL: [[u32; 4]; 3] =
    [[NONE, NONE, NONE, NONE], [0xa5d058f8, 0xa5d05904, 0xa5d058fe, 0x4a60ce95], [0xa5d059fd, 0xa5d059f1, 0xa5d059f7, 0xa5d059fd]];
pub const CK_DEATH_POSE: [[u32; 4]; 3] =
    [[NONE, NONE, NONE, NONE], [0x4a60ce97, 0xa5d058fd, 0xa5d05903, 0xa5d058f6], [0xa5d059ff, 0xa5d059fa, 0xa5d059f4, 0xa5d059ff]];
pub const CK_DEATH: [[u32; 4]; 3] =
    [[NONE, NONE, NONE, NONE], [0x4a60ce98, 0xa5d058fc, 0xa5d05902, 0xa5d058f7], [0xa5d05a00, 0xa5d059f9, 0xa5d059f3, 0xa5d05a00]];
/// The counter dodge [weapon][the side it goes: -, left, right, back][foot] (0x1A32460, 0xF45CF0).
pub const COUNTER_DODGE: [[[u32; 2]; 4]; 3] = [
    [[NONE, NONE], [0x1cd34d97, 0x1cd34d97], [0x1cd34d99, 0x1cd34d99], [0x1cd34d95, 0x1cd34d95]],
    [[NONE, NONE], [0x1cd34d97, 0x1cd34d97], [0x1cd34d99, 0x1cd34d99], [0x1cd34d95, 0x1cd34d95]],
    [[NONE, NONE], [0x2c16898a, 0x2c16898a], [0x2c16898b, 0x2c16898b], [0x2c168989, 0x2c168989]],
];
/// A strike deflected: the attacker's recoil [weapon][direction][foot] (0x1A32738, 0xF46090).
pub const DEFLECTED: [[[u32; 2]; 4]; 3] = [
    [[0x1cd3500a, 0x1cd3500a], [0x168fc3c7, 0x168fc3c7], [0x1cd35008, 0x1cd35008], [0x0f3b8c12, 0x11a7016e]],
    [[0x1cd3500a, 0x2949b208], [0x168fc3c7, 0x168fc3c7], [0x1cd35008, 0x2949b20a], [0x0f3b8c12, 0x11a7016e]],
    [[0x1cd3500a, 0x1cd3500a], [0x168fc3c7, 0x168fc3c7], [0x1cd35008, 0x1cd35008], [0x0f3b8c12, 0x11a7016e]],
];
/// The counter kill: the defender's step in and kill [its weapon][kill, slap][the attacker's weapon][direction][its foot]
/// (0x1A32E10 played by 0xF4A2B0, 0x1A33118 by 0xF4A9A0): the left foot ahead the `_01` set, the right the `_02`.
pub const COUNTER_STEP: [[[[[u32; 2]; 4]; 3]; 2]; 3] = [
    [
        [
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
        ],
        [
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
        ],
    ],
    [
        [
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[0x0f9292a0, 0x0f92a9c3], [0x27ce227c, 0x27ce2282], [0x168fc4cc, 0x27ce2285], [0x27ce227f, 0x27ce2288]],
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
        ],
        [
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[0x3a5c526a, 0x1cd37a45], [0x3a5c5264, 0x3a5c5270], [0x3a5c5267, 0x3a5c5273], [0x3a5c526d, 0x3a5c5276]],
            [[0x3a5c526a, 0x1cd37a45], [0x3a5c5264, 0x3a5c5270], [0x3a5c5267, 0x3a5c5273], [0x3a5c526d, 0x3a5c5276]],
        ],
    ],
    [
        [
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[0x2c169959, 0x2c16995f], [0x2f2f2df8, 0x2f2f2dfb], [0x2f2f2c17, 0x2f2f2c1a], [0x2c16995c, 0x2c169962]],
            [[0x2c169959, 0x2c16995f], [0x2f2f2df8, 0x2f2f2dfb], [0x2f2f2c17, 0x2f2f2c1a], [0x2c16995c, 0x2c169962]],
        ],
        [
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[0x3a5c526a, 0x1cd37a45], [0x3a5c5264, 0x3a5c5270], [0x3a5c5267, 0x3a5c5273], [0x3a5c526d, 0x3a5c5276]],
            [[0x3a5c526a, 0x1cd37a45], [0x3a5c5264, 0x3a5c5270], [0x3a5c5267, 0x3a5c5273], [0x3a5c526d, 0x3a5c5276]],
        ],
    ],
];
pub const COUNTER_KILL: [[[[[u32; 2]; 4]; 3]; 2]; 3] = [
    [
        [
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
        ],
        [
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
        ],
    ],
    [
        [
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[0x0f9292a1, 0x0f92a9c4], [0x27ce227d, 0x27ce2283], [0x168fc4cd, 0x27ce2286], [0x27ce2280, 0x27ce2289]],
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
        ],
        [
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[0x3a5c526b, 0x1cd37a46], [0x3a5c5265, 0x3a5c5271], [0x3a5c5268, 0x3a5c5274], [0x3a5c526e, 0x3a5c5277]],
            [[0x3a5c526b, 0x1cd37a46], [0x3a5c5265, 0x3a5c5271], [0x3a5c5268, 0x3a5c5274], [0x3a5c526e, 0x3a5c5277]],
        ],
    ],
    [
        [
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[0x2c16995a, 0x2c169960], [0x2f2f2df9, 0x2f2f2dfc], [0x2f2f2c18, 0x2f2f2c1b], [0x2c16995d, 0x2c169963]],
            [[0x2c16995a, 0x2c169960], [0x2f2f2df9, 0x2f2f2dfc], [0x2f2f2c18, 0x2f2f2c1b], [0x2c16995d, 0x2c169963]],
        ],
        [
            [[NONE, NONE], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
            [[0x3a5c526b, 0x1cd37a46], [0x3a5c5265, 0x3a5c5271], [0x3a5c5268, 0x3a5c5274], [0x3a5c526e, 0x3a5c5277]],
            [[0x3a5c526b, 0x1cd37a46], [0x3a5c5265, 0x3a5c5271], [0x3a5c5268, 0x3a5c5274], [0x3a5c526e, 0x3a5c5277]],
        ],
    ],
];
/// Attacks at a target over 0.5 m higher or lower (the attack kind 3): [weapon][distance][low, high][strong, max][foot]
/// (0x1A31C38) and their strikes [strong, max][weapon][distance][low, high][foot] (0x1A342D8, 0x1A34488; 0xF40460).
pub const HIGHLOW_START: [[[[[u32; 2]; 2]; 2]; 3]; 3] = [
    [
        [[[NONE, NONE], [NONE, NONE]], [[NONE, NONE], [NONE, NONE]]],
        [[[NONE, NONE], [NONE, NONE]], [[NONE, NONE], [NONE, NONE]]],
        [[[NONE, NONE], [NONE, NONE]], [[NONE, NONE], [NONE, NONE]]],
    ],
    [
        [[[0x22cadc75, 0x22cadc78], [0x22cadc75, 0x22cadc78]], [[0x22cadc6f, 0x22cadc72], [0x22cadc6f, 0x22cadc72]]],
        [[[0x22cadc63, 0x22cadc6c], [0x22cadc63, 0x22cadc6c]], [[0x22caea9d, 0x2715ce63], [0x22caea9d, 0x2715ce63]]],
        [[[0x22cadc63, 0x22cadc6c], [0x22cadc63, 0x22cadc6c]], [[0x22caea9d, 0x2715ce63], [0x22caea9d, 0x2715ce63]]],
    ],
    [
        [[[0x22cadc75, 0x22cadc78], [0x22cadc75, 0x22cadc78]], [[0x22cadc6f, 0x22cadc72], [0x22cadc6f, 0x22cadc72]]],
        [[[0x22cadc63, 0x22cadc6c], [0x22cadc63, 0x22cadc6c]], [[0x22caea9d, 0x2715ce63], [0x22caea9d, 0x2715ce63]]],
        [[[0x22cadc63, 0x22cadc6c], [0x22cadc63, 0x22cadc6c]], [[0x22caea9d, 0x2715ce63], [0x22caea9d, 0x2715ce63]]],
    ],
];
pub const HIGHLOW_STRIKE: [[[[[Strike; 2]; 2]; 3]; 3]; 2] = [
    [
        [[[(NONE, 0), (NONE, 0)], [(NONE, 0), (NONE, 0)]], [[(NONE, 0), (NONE, 0)], [(NONE, 0), (NONE, 0)]], [[(NONE, 0), (NONE, 0)], [(NONE, 0), (NONE, 0)]]],
        [
            [[(0x22cadc76, 0), (0x22cadc79, 3)], [(0x22cadc70, 1), (0x22cadc73, 2)]],
            [[(0x22cadc64, 0), (0x22cadc6d, 3)], [(0x22caea9e, 1), (0x2715ce64, 3)]],
            [[(0x22cadc64, 0), (0x22cadc6d, 3)], [(0x22caea9e, 1), (0x2715ce64, 3)]],
        ],
        [
            [[(0x22cadc76, 0), (0x22cadc79, 3)], [(0x22cadc70, 1), (0x22cadc73, 2)]],
            [[(0x22cadc64, 0), (0x22cadc6d, 3)], [(0x22caea9e, 1), (0x2715ce64, 3)]],
            [[(0x22cadc64, 0), (0x22cadc6d, 3)], [(0x22caea9e, 1), (0x2715ce64, 3)]],
        ],
    ],
    [
        [[[(NONE, 0), (NONE, 0)], [(NONE, 0), (NONE, 0)]], [[(NONE, 0), (NONE, 0)], [(NONE, 0), (NONE, 0)]], [[(NONE, 0), (NONE, 0)], [(NONE, 0), (NONE, 0)]]],
        [
            [[(0x22cadc76, 0), (0x22cadc79, 3)], [(0x22cadc70, 1), (0x22cadc73, 2)]],
            [[(0x22cadc64, 0), (0x22cadc6d, 3)], [(0x22caea9e, 1), (0x2715ce64, 3)]],
            [[(0x22cadc64, 0), (0x22cadc6d, 3)], [(0x22caea9e, 1), (0x2715ce64, 3)]],
        ],
        [
            [[(0x22cadc76, 0), (0x22cadc79, 3)], [(0x22cadc70, 1), (0x22cadc73, 2)]],
            [[(0x22cadc64, 0), (0x22cadc6d, 3)], [(0x22caea9e, 1), (0x2715ce64, 3)]],
            [[(0x22cadc64, 0), (0x22cadc6d, 3)], [(0x22caea9e, 1), (0x2715ce64, 3)]],
        ],
    ],
];
/// Mobile attacks (on the run): [weapon][kind: 0 drawn, 3 drawing as it strikes, ...][foot] (0x1A32220, 0xF49DB0) and
/// their strikes (records at 0x1A34908, 0xF40460).
pub const MOBILE_START: [[[u32; 2]; 7]; 3] = [
    [[0x27e22468, 0x27e22473], [NONE, NONE], [NONE, NONE], [0x27e22468, 0x27e22473], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
    [
        [0x09acfb63, 0x09acf747],
        [0x2200956b, 0x220095a4],
        [0x2200956b, 0x220095a4],
        [0x135241af, 0x17e0bbc3],
        [0x220095bf, 0x220095eb],
        [0x220095bf, 0x220095eb],
        [0x00000000, 0x00000000],
    ],
    [[0x2c169386, 0x2c169387], [NONE, NONE], [NONE, NONE], [0xa5d05742, 0xa5d05743], [NONE, NONE], [NONE, NONE], [NONE, NONE]],
];
pub const MOBILE_STRIKE: [[[Strike; 2]; 7]; 3] = [
    [
        [(0x27e22469, 3), (0x27e22474, 3)],
        [(NONE, 0), (NONE, 0)],
        [(NONE, 0), (NONE, 0)],
        [(0x27e22469, 3), (0x27e22474, 3)],
        [(NONE, 0), (NONE, 0)],
        [(NONE, 0), (NONE, 0)],
        [(NONE, 0), (NONE, 0)],
    ],
    [
        [(0x09acfb65, 3), (0x09acf748, 3)],
        [(0x220095ad, 3), (0x220095b1, 3)],
        [(0x2200956c, 3), (0x220095a0, 3)],
        [(0x135241b0, 1), (0x17e0bbc4, 1)],
        [(0x220095d3, 1), (0x220095d7, 1)],
        [(0x220095c0, 1), (0x220095c4, 1)],
        [(0xebf467f4, 1), (0xebf46889, 1)],
    ],
    [
        [(0x2c16937c, 3), (0x2c169381, 3)],
        [(0x2c169377, 3), (0x2c1693a9, 3)],
        [(0x2c16936d, 3), (0x2c169372, 3)],
        [(0xa5d05744, 3), (0xa5d05745, 3)],
        [(0x2c169377, 3), (0x2c1693a9, 3)],
        [(0x2c16936d, 3), (0x2c169372, 3)],
        [(0xebf46e99, 1), (0xebf46e9a, 1)],
    ],
];
/// The stance's waits, the defence's waits, the defence's and the stance's walks (8-way blend spaces) [weapon, 3 the
/// hidden blade][foot] (0x1A31760, 0x1A31784, 0x1A317A8, 0x1A317B0). The sword's light wait lists no items, only idle
/// variants: `combat` plays its clip by name.
pub const WAIT: [[u32; 2]; 4] = [[0x068d7393, 0x068d7394], [0x0108295c, 0x00fd645b], [0x1c0a0a9c, 0x1c0a0a9d], [0x089ca74c, 0x089ca74d]];
pub const DEFENCE_WAIT: [[u32; 2]; 4] = [[0x068d799e, 0x068d799f], [0x1122a264, 0x1122a265], [0x2c1692a2, 0x2c1692a3], [0x089ca74a, 0x089ca74b]];
pub const DEFENCE_STRAFE: [[u32; 2]; 4] = [[0x869b2977, 0x869b2978], [0x1122a263, 0x1122a262], [0x2f2f3561, 0x2f2f3562], [0x69b54b98, 0x69b54b99]];
pub const STRAFE: [[u32; 2]; 4] = [[0x0ed89b1d, 0x0ed89b1e], [0x0429c55b, 0x0429dbae], [0x2c168903, 0x2c168904], [0x69b54aeb, 0x69b54aec]];

/// The attack's anticipation [weapon][foot] (0x1A31904, played by 0xF36F00 as the attack state's first sub-state
/// +0xB18): `xx_h_<style>_front_atk_anticipation_<foot>`, 0.2 s for the fists and the sword, 0.07 s for the short blade.
pub const ANTICIPATION: [[u32; 2]; 3] = [[0x1bdd4cf2, 0x1bdd4cf1], [0x1227b570, 0x1122a1fd], [0x2c16994b, 0x2c16994c]];

/// The counter on a max attack (`HumanGround_CounterStrong`, the counter state's type 3, played by 0xF4A2B0): the
/// defender's step in, its parry (`_a` then `_b`) and its branch [the attacker's blade direction][the defender's foot]
/// (0x1A3341C, 0x1A33440, 0x1A33464): the `01` set off the left foot, `02` off the right.
pub const COUNTER_MAX_START: [[u32; 2]; 4] = [[0xaf8626aa, 0xb4f3cd88], [0xb4f3cdf3, 0xb4f3ce02], [0xb4f3ce11, 0xb4f3ce20], [0xb4f3cd9b, 0xb4f3cdaa]];
pub const COUNTER_MAX_PARRY: [[u32; 2]; 4] = [[0xaf8626ab, 0xb4f3cd89], [0xb4f3cdf4, 0xb4f3ce03], [0xb4f3ce12, 0xb4f3ce21], [0xb4f3cd9c, 0xb4f3cdab]];
pub const COUNTER_MAX_BRANCH: [[u32; 2]; 4] = [[0xaf8626ac, 0xb4f3cd8a], [0xb4f3cdf5, 0xb4f3ce04], [0xb4f3ce13, 0xb4f3ce22], [0xb4f3cd9d, 0xb4f3cdac]];
/// The finish after the branch [the defender's weapon: fists' kick, sword, short blade][its foot][variant] (0x1A33488).
pub const COUNTER_MAX_FINISH: [[[u32; 2]; 2]; 3] =
    [[[0xaf8626b3, NONE], [0xb4f3ce36, NONE]], [[0xaf8626b7, 0x1c26dd43], [0xb4f3ce38, 0x1c26dd46]], [[0xc2830344, 0x1c26dd3d], [0xc2830348, 0x1c26dd40]]];
/// The attacker's side: countered (`_a` then `_b`) and stunned [its blade direction][the set, 01 / 02] (0x1A334BC,
/// 0x1A334E0), and its death to the finish [the defender's weapon][set][variant] (0x1A33504).
pub const COUNTERED_MAX: [[u32; 2]; 4] = [[0xb4f3cd9f, 0xb4f3cded], [0xb4f3ce15, 0xb4f3ce24], [0xb4f3cdf7, 0xb4f3ce06], [0xaf8626b0, 0xb4f3cd95]];
pub const COUNTERED_MAX_STUNNED: [[u32; 2]; 4] = [[0xb4f3cda0, 0xb4f3cdee], [0xb4f3ce16, 0xb4f3ce25], [0xb4f3cdf8, 0xb4f3ce07], [0xaf8626b1, 0xb4f3cd96]];
pub const COUNTERED_MAX_FINISH: [[[u32; 2]; 2]; 3] =
    [[[0xaf8626b5, NONE], [0xb4f3ce3b, NONE]], [[0xaf8626b9, 0x1c26dd45], [0xb4f3ce3c, 0x1c26dd48]], [[0xc2830347, 0x1c26dd3f], [0xc2830346, 0x1c26dd42]]];

/// The hidden blade's counter kill (the counter kill tables' 4th weapon, 0xF4A2B0 / 0xF44B40 index it 3 for the fists
/// with the hidden blade out, +0x974): its step in and kill [the attacker's blade direction][its foot] (0x1A32E10's and
/// 0x1A33118's hidden blade rows: `xx_h_blade_counter_light_<01|02>_kill_<dir>_step_start` / `_position`), and the
/// victim's death, which the block pairs with the kill's mirror direction (ul with ur, dl with dr).
pub const BLADE_COUNTER_STEP: [[u32; 2]; 4] = [[0x0ef67b19, 0x0ef67b1d], [0x0ef67b10, 0x0ef679e0], [0x0ef679e4, 0x0ef67a82], [0x073806c8, 0x089ca09c]];
pub const BLADE_COUNTER_KILL: [[u32; 2]; 4] = [[0x0ef67b1a, 0x0ef67b1e], [0x0ef67b11, 0x0ef679e1], [0x0ef679e5, 0x0ef67a83], [0x073806c9, 0x089ca09d]];
pub const BLADE_COUNTERED: [[u32; 2]; 4] = [[0x0ef67b1c, 0x0ef67b22], [0x0ef67b13, 0x0ef679e3], [0x0ef679e7, 0x0ef67a85], [0x073806cb, 0x089ca09f]];

/// The grab (`HumanGround_Fight`): the grabber's start and pull, its throw by the side it goes [front, left, right,
/// back] (0x1A318C0, the same for every weapon); the grabbed one's pull and throw (the actions the grab's items pair
/// with), and the thrown one's fall and roll back up (`HumanGround_Hurt` 0xFA49213: its first item is 0x31663E96's, the
/// action the throw links to).
pub const GRAB_START: u32 = 0x3266a567;
pub const GRAB_PULL: u32 = 0x3266a56a;
pub const GRAB_THROW: [u32; 4] = [0x3266a56c, 0x3266a572, 0x3266a575, 0x3266a56f];
pub const GRABBED_PULL: u32 = 0x3266a578;
pub const GRABBED_THROW: [u32; 4] = [0x3266a57a, 0x3266a57e, 0x3266a580, 0x3266a57c];
pub const THROWN: [u32; 1] = [0x0fa49213];
