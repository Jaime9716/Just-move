"""Constantes del juez v1 (contracts/judge_algorithm_v1.md) y fusión de configuración.

Las claves van en minúsculas. `merge_config` acepta también las mayúsculas del contrato
(p. ej. "PEAK_MIN") y lanza ValueError ante claves desconocidas.
"""

G = 9.80665

LIMBS = ("left_hand", "right_hand", "left_foot", "right_foot")
LIMB_ORDER = {name: i for i, name in enumerate(LIMBS)}
FOOT_LIMBS = ("left_foot", "right_foot")

DEFAULT_CONFIG = {
    # B3
    "g": G,
    "lpf_cutoff_hz": 20.0,
    "comp_alpha": 0.02,
    "gravity_gate_lo": 0.8,     # × G
    "gravity_gate_hi": 1.2,     # × G
    # B4
    "frame_win_s": 0.100,
    "frame_hop_s": 0.020,
    "peak_min": 8.0,
    "peak_rel": 1.5,
    "peak_rel_window_s": 1.0,
    "peak_min_sep_s": 0.150,
    "dir_min_norm": 0.5,
    # B5
    "perfect_window_ms": 50.0,
    "good_window_ms": 100.0,
    "foot_window_scale": 1.5,
    "w_ncc": 0.6,
    "w_dir": 0.4,
    "energy_gate": 0.25,
    "noref_energy_full": 6.0,
    "perfect_sim": 0.75,
    "good_sim": 0.5,
    "emit_margin_s": 0.05,
    # ---- Extensiones (NO son v1; ver proto/ALGORITHM.md). Con estos valores = v1 exacto.
    # Multiplicador del rango de búsqueda de desfase L en "move": v1 = 1.0 (±gw).
    "lag_search_scale": 1.0,
}

# Variantes con nombre (ver proto/ALGORITHM.md). "v1" es el contrato.
VARIANTS = {
    "v1": {},
    "v1b": {"lag_search_scale": 2.0},
}


def merge_config(config=None, variant=None):
    """Devuelve un dict con DEFAULT_CONFIG + variante + `config` (claves en minúsculas)."""
    cfg = dict(DEFAULT_CONFIG)
    if variant is not None:
        if variant not in VARIANTS:
            raise ValueError(f"variante desconocida: {variant!r} (hay {sorted(VARIANTS)})")
        cfg.update(VARIANTS[variant])
    if config:
        # la variante se aplica primero; las claves explícitas mandan sobre ella
        for key, value in config.items():
            if key.lower() == "variant":
                if value not in VARIANTS:
                    raise ValueError(f"variante desconocida: {value!r}")
                cfg.update(VARIANTS[value])
        for key, value in config.items():
            k = key.lower()
            if k == "variant":
                continue
            if k == "gravity_gate":  # admite (lo, hi)
                cfg["gravity_gate_lo"], cfg["gravity_gate_hi"] = float(value[0]), float(value[1])
                continue
            if k not in DEFAULT_CONFIG:
                raise ValueError(f"clave de configuración desconocida: {key!r}")
            cfg[k] = float(value)
    return cfg


def resolve_config(config=None):
    """`config` ya fusionado (tiene todas las claves) se usa tal cual; si no, se fusiona."""
    if config is not None and all(k in config for k in DEFAULT_CONFIG) and "variant" not in config:
        return config
    return merge_config(config)
