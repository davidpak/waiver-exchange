Fair Price 2.0 — Adaptive, Performance-Weighted Reference Price (RPE)

Purpose: replace the projection-anchored fair price with an adaptive blend that (1) starts from preseason projections, (2) rapidly incorporates actual season performance pace, and (3) adds smoothed momentum from recent weeks. Result: breakout players climb quickly; laggards decay; flukes are damped.

This spec is written so a developer can implement it directly inside the existing RPE service with no new external APIs beyond the two we already use.

0) Inputs & Data We Already Have

External sources (unchanged):

PlayerSeasonProjectionStats/{season} → PlayerID, FantasyPoints (or FantasyPointsPPR), FantasyPosition.

PlayerGameStatsByWeek/{season}/{week} → PlayerID, FantasyPoints for each week.

Derived internal state per player (new/explicit):

struct PlayerPerf {
  player_id: i64,
  fantasy_pos: FantasyPos,   // QB, RB, WR, TE
  proj_points: f64,          // P_proj (season projection)
  actual_points: f64,        // Σ FantasyPoints up to current week
  weeks_played: u8,          // number of non-bye weeks with data
  last_week_points: f64,     // FantasyPoints of the most recent completed week
  ema_delta_points: f64,     // EMA over Δpts (recent momentum)
  f0_cents: i64,             // initial baseline at t0 (kept for band reference)
  fair_cents: i64,           // current F_t
}


Config (new keys added to existing market.yml):

fair2:
  base_cents: 5000                 # $50
  beta_cents_per_pt: 300           # $3 per point -> applied to blended-per-week
  kappa_cents_per_pt:
    QB: 100                        # $1 per point
    RB: 150
    WR: 150
    TE: 150
  alpha_mode: "linear"             # "linear" | "exp"
  alpha_exp_lambda: 0.12           # only if alpha_mode="exp" : α = e^(-λ * weeks_played)
  band_bps: 3000                   # ±30% clip vs F0
  ema_delta:
    window: 3                      # conceptual window; we use smoothing factor below
    smoothing: 0.3                 # EMA smoothing factor for Δpts (0<γ≤1)
  consistency:
    enabled: true
    scale: 10.0                    # κ_adj = κ / (1 + σ / scale)
    min_weeks_for_sigma: 4

1) Definitions

P_proj: preseason projected fantasy points for the season.

W_total: 17 (regular season).

P_actual: cumulative fantasy points scored to date (sum of weekly).

W_played: number of weeks with an actual stat line (exclude byes and DNP).

P_pace: P_actual / W_played * 17 (season pace from realized performance).

α(W): weight on projections. Two choices:

Linear: α = max(0, 1 − W_played / 17).

Exponential: α = exp(−λ · W_played) with λ from config.

P_blend: α · P_proj + (1 − α) · P_pace.

Δpts_week: FantasyPoints(week t) − FantasyPoints(week t−1).

EMA_Δ: exponential moving average of Δpts_week with smoothing γ (config).

2) Formulas
2.1 Baseline from blended projection

Convert season blended to weekly and then to baseline dollars:

P_blend_per_week
=
𝑃
𝑏
𝑙
𝑒
𝑛
𝑑
17
P_blend_per_week=
17
P
blend
	​

	​

𝐹
𝑏
𝑎
𝑠
𝑒
=
base
+
𝛽
⋅
P_blend_per_week
F
base
	​

=base+β⋅P_blend_per_week

Where base = $50, β = $3 (tunable).

2.2 Smoothed momentum from recent weeks

Compute weekly change and maintain an EMA:

Δ
𝑃
𝑤
𝑒
𝑒
𝑘
=
𝑃
𝑤
𝑒
𝑒
𝑘
(
𝑡
)
−
𝑃
𝑤
𝑒
𝑒
𝑘
(
𝑡
−
1
)
ΔP
week
	​

=P
week
	​

(t)−P
week
	​

(t−1)
𝐸
𝑀
𝐴
Δ
(
𝑡
)
=
𝛾
⋅
Δ
𝑃
𝑤
𝑒
𝑒
𝑘
+
(
1
−
𝛾
)
⋅
𝐸
𝑀
𝐴
Δ
(
𝑡
−
1
)
EMA
Δ
	​

(t)=γ⋅ΔP
week
	​

+(1−γ)⋅EMA
Δ
	​

(t−1)

Map to dollars using κ by position:

𝐹
𝑚
𝑜
𝑚
=
𝜅
(
𝑝
𝑜
𝑠
)
⋅
𝐸
𝑀
𝐴
Δ
(
𝑡
)
F
mom
	​

=κ(pos)⋅EMA
Δ
	​

(t)

Optionally adjust κ by consistency (volatility):

If enough weeks (≥ min_weeks_for_sigma), compute sample stddev σ of weekly points; then

𝜅
𝑎
𝑑
𝑗
=
𝜅
1
+
𝜎
/
scale
κ
adj
	​

=
1+σ/scale
κ
	​


Use κ_adj in place of κ.

2.3 Final fair price with band clip

Use the original F₀ (f0_cents) as the band anchor:

𝐹
𝑡
\*
=
𝐹
𝑏
𝑎
𝑠
𝑒
+
𝐹
𝑚
𝑜
𝑚
F
t
\*
	​

=F
base
	​

+F
mom
	​

𝐹
𝑡
=
𝑐
𝑙
𝑖
𝑝
(
𝐹
𝑡
\*
,
 
𝑓
0
_
𝑐
𝑒
𝑛
𝑡
𝑠
⋅
(
1
−
𝑏
𝑎
𝑛
𝑑
)
,
 
𝑓
0
_
𝑐
𝑒
𝑛
𝑡
𝑠
⋅
(
1
+
𝑏
𝑎
𝑛
𝑑
)
)
F
t
	​

=clip(F
t
\*
	​

, f0_cents⋅(1−band), f0_cents⋅(1+band))

where band = band_bps / 10_000 (e.g., 0.30 for ±30%).

3) Update Cycle (when weekly stats poll arrives)

For each (player_id) present in the payload for the configured season, week:

Accumulate actuals

Pull this week’s FantasyPoints (0 if bye or inactive but only count W_played if Played=1 or FP>0 depending on preferred rule).

Update:

P_actual += FP_week

If FP_week > 0 (or if Played=1), W_played += 1

Momentum

Δpts_week = FP_week - last_week_points

ema_delta_points = γ*Δpts_week + (1−γ)*ema_delta_points

last_week_points = FP_week

Blend

Compute P_pace = (W_played > 0) ? P_actual / W_played * 17 : P_proj

Compute α (linear or exponential from config)

P_blend = α*P_proj + (1−α)*P_pace

Base + Momentum

F_base = base + β * (P_blend/17)

Determine κ from position; adjust by σ if consistency enabled and enough weeks.

F_mom = κ_adj * ema_delta_points

Final

F*_t = F_base + F_mom

F_t = clip(F*_t, f0_cents*(1−band), f0_cents*(1+band))

Emit & Persist

If |F_t − prev_fair_cents| >= 1 (tick), persist to rpe_fair_prices:

fair_cents, band_bps, kappa_cents_per_pt, actual_pts=P_actual,
delta_pts=Δpts_week, reason={"fair2": {"alpha": α, "blend": P_blend, "pace": P_pace, "ema_delta": ema_delta_points}}

Broadcast RpeFairPriceUpdate(player_id, fair_cents).

Note: Between polls you may still run the lightweight UI pacing you already have (e.g., “step” or “poll-step”) purely for visuals; it must not affect the stored fair_cents—that only updates on true data.

4) Initialization & Persistence
4.1 On season boot (pre-games)

Load P_proj for all players from projections_season.

Initialize:

P_actual=0, W_played=0, ema_delta_points=0, last_week_points=0.

Compute α(0)=1.0 → P_blend=P_proj.

F_base = base + β*(P_proj/17).

Set f0_cents = F_base.round().

Set current fair_cents = f0_cents and persist initial rpe_fair_prices rows (reason={"projection":true}).

4.2 On service restart

Rehydrate from DB:

projections_season for P_proj/position.

player_week_points to recompute P_actual, W_played, weekly series (for σ), last_week_points, and ema_delta_points.

Last rpe_fair_prices row for the current fair_cents (optional; derived anyway).

Store per-player state in memory for fast updates.

5) Edge Cases & Policy

Players with 0 weeks played: set P_pace = P_proj until first week appears; α still computed but has no effect until W_played>0.

Byes: do not increment W_played nor Δpts_week (0 week). EMA carries forward via (1−γ)*prev.

Late activation/breakouts: exponential α will adapt faster; if using linear α and want faster reweighting, switch to exp with λ≈0.12–0.18.

Injuries/DNP: if Played=0 but FP>0 (rare), treat as played; otherwise do not increment W_played. (Configurable if you prefer counting a week with Played=1 even with 0 FP.)

Band exhaustion: If a superstar exceeds the ±30% band early, (a) consider bigger band_bps (e.g., 4000) or (b) rebase f0_cents weekly (advanced; v2).

Positions (κ): start with QB=100, RB/WR/TE=150; fine-tune by observed volatility.

Consistency σ: compute rolling stddev of weekly FP over last min(6, W_played) weeks to avoid stale early-season σ. Disable if adds noise.

6) Database Notes

No schema changes required if you already have the spec’s tables. The following values should be persisted to rpe_fair_prices on true updates:

fair_cents, band_bps, kappa_cents_per_pt, actual_pts (= P_actual), delta_pts (= Δpts_week), pacing_mode (still record "step" or "poll-step" for UI), and:

reason: {
  "fair2": {
    "alpha": 0.41,
    "blend": 212.7,
    "pace": 265.3,
    "ema_delta": 6.8
  }
}


If you want long-term analytics, you can add a small table to snapshot internal state weekly (optional).

7) Pseudocode (drop-in for RPE update handler)
fn alpha_linear(w_played: u8) -> f64 {
    (1.0 - (w_played as f64 / 17.0)).max(0.0)
}

fn alpha_exp(w_played: u8, lambda: f64) -> f64 {
    (-lambda * (w_played as f64)).exp()
}

fn kappa_for(pos: FantasyPos, cfg: &Cfg) -> i64 {
    match pos {
        FantasyPos::QB => cfg.kappa.QB,
        FantasyPos::RB => cfg.kappa.RB,
        FantasyPos::WR => cfg.kappa.WR,
        FantasyPos::TE => cfg.kappa.TE,
    }
}

fn update_player_after_week(p: &mut PlayerPerf, week_points: f64, cfg: &Cfg) -> i64 {
    // 1) Actuals + weeks
    let played_this_week = week_points > 0.0; // or use Played==1 from payload
    p.actual_points += week_points;
    if played_this_week { p.weeks_played = p.weeks_played.saturating_add(1); }

    // 2) Momentum EMA
    let delta = week_points - p.last_week_points;
    p.ema_delta_points = cfg.ema.smoothing * delta + (1.0 - cfg.ema.smoothing) * p.ema_delta_points;
    p.last_week_points = week_points;

    // 3) Blend projection with pace
    let p_pace = if p.weeks_played > 0 {
        (p.actual_points / p.weeks_played as f64) * 17.0
    } else {
        p.proj_points
    };

    let alpha = if cfg.alpha_mode == "exp" {
        alpha_exp(p.weeks_played, cfg.alpha_exp_lambda)
    } else {
        alpha_linear(p.weeks_played)
    };

    let p_blend = alpha * p.proj_points + (1.0 - alpha) * p_pace;

    // 4) Base + momentum
    let f_base = cfg.base_cents as f64 + cfg.beta_cents_per_pt as f64 * (p_blend / 17.0);

    let mut kappa = kappa_for(p.fantasy_pos, cfg) as f64;

    if cfg.consistency.enabled && p.weeks_played >= cfg.consistency.min_weeks_for_sigma {
        let sigma = compute_recent_sigma(p.player_id); // your rolling stddev of weekly FP
        kappa = kappa / (1.0 + sigma / cfg.consistency.scale);
    }

    let f_mom = kappa * p.ema_delta_points;
    let f_star = f_base + f_mom;

    // 5) Clip to band vs F0
    let band = cfg.band_bps as f64 / 10_000.0;
    let lower = (p.f0_cents as f64) * (1.0 - band);
    let upper = (p.f0_cents as f64) * (1.0 + band);
    let f_new = f_star.clamp(lower, upper).round() as i64;

    p.fair_cents = f_new;
    f_new
}


Broadcast only when |f_new - prev| >= 1 (a tick).

8) Testing Plan

Unit

Blend behavior: as W_played increases, P_blend moves from P_proj toward P_pace.

Momentum: given a weekly points sequence, EMA evolves correctly with γ.

κ-adjustment: higher weekly σ → lower effective κ.

Golden Cases

Breakout: proj=120, weeks: 25,25,20,30
Expect: P_pace ≈ 340; α ~ 0.76 (wk4 linear) → P_blend rises; F_base + positive F_mom → large lift, clipped by band if necessary.

Laggard: proj=220, weeks: 5,8,7,4
Expect: P_pace ≈ 136; P_blend falls; F_base declines; F_mom small negative; price decays.

Fluke: sequence 2,3,30
Expect: big Δ at week 3 but EMA smooths; price increases but not to absurd levels.

Zero weeks: player inactive → P_blend=P_proj; F_t=F0.

Integration

Backfill a fake month of player_week_points, run RPE, confirm rpe_fair_prices monotonicity + band compliance + reasonable ordering of top performers.

Performance

O(n) over players each poll; memory bounded by roster size; no extra I/O beyond existing writes.

9) Operational Guidance

Start with linear α for simplicity; switch to exp α if you want faster midseason adaptation without re-tuning β/κ.

Begin with band = ±30%; if too tight for superstars, consider ±40% or (advanced) rebase f0_cents after week 8.

Tune β and κ off a historical backtest (one afternoon effort):

Objective: keep typical weekly move within a visually pleasing range; top-10 players rank near top by price after 3–5 weeks.

10) What Changes for Other Services?

Router/House/EVS: no changes beyond consuming the new fair_cents. All admission/collars still reference Fₜ.

Frontend: no API change; charts will naturally show stronger separation of high performers from projections-darlings who underperform.

TL;DR

Fₜ = Base(from an adaptive blend of projection & actual pace) + Smoothed Momentum(EMA of weekly Δpts) → clipped by ±30% vs F₀.
No new endpoints; a handful of per-player fields; deterministic; fast; and—most importantly—it prices stars like stars when they actually play like stars.