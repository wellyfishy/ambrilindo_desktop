use std::collections::HashMap;
use rand::seq::SliceRandom;
use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Quadrant {
    Q1,
    Q2,
    Q3,
    Q4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Half {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SlotPosition {
    Atlet1,
    Atlet2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MatchSlot {
    pub match_num: u32, // 1..=8
    pub position: SlotPosition,
}

pub fn quadrant_of_match(m: u32) -> Quadrant {
    match m {
        1 | 2 => Quadrant::Q1,
        3 | 4 => Quadrant::Q2,
        5 | 6 => Quadrant::Q3,
        7 | 8 => Quadrant::Q4,
        _ => Quadrant::Q1,
    }
}

pub fn half_of_match(m: u32) -> Half {
    match m {
        1..=4 => Half::Top,
        _ => Half::Bottom,
    }
}

pub fn primary_slot(m: u32) -> SlotPosition {
    match m {
        1 | 3 | 5 | 7 => SlotPosition::Atlet1,
        _ => SlotPosition::Atlet2,
    }
}

pub fn secondary_slot(m: u32) -> SlotPosition {
    match m {
        1 | 3 | 5 | 7 => SlotPosition::Atlet2,
        _ => SlotPosition::Atlet1,
    }
}

pub fn quadrant_matches(q: Quadrant) -> (u32, u32) {
    // (outer, inner)
    match q {
        Quadrant::Q1 => (1, 2),
        Quadrant::Q2 => (4, 3),
        Quadrant::Q3 => (5, 6),
        Quadrant::Q4 => (8, 7),
    }
}

/// Tree distance between two bracket slots s1 and s2:
/// 4: Opposite halves (meet in Final)
/// 3: Same half, opposite quadrants (meet in Semifinal)
/// 2: Same quadrant, different matches (meet in Quarterfinal)
/// 1: Same match (Round 1 opponent - maximum collision!)
pub fn slot_distance(s1: &MatchSlot, s2: &MatchSlot) -> u32 {
    if s1.match_num == s2.match_num {
        return 1;
    }
    if quadrant_of_match(s1.match_num) == quadrant_of_match(s2.match_num) {
        return 2;
    }
    if half_of_match(s1.match_num) == half_of_match(s2.match_num) {
        return 3;
    }
    4
}

/// Penalty assigned to distance between two competitors from the same club/contingent
pub fn slot_penalty(s1: &MatchSlot, s2: &MatchSlot) -> u64 {
    match slot_distance(s1, s2) {
        1 => 1_000_000,
        2 => 10_000,
        3 => 100,
        _ => 0,
    }
}

/// Generates balanced slot assignments across halves and quadrants with uniform entropy.
/// Direct replica of legacy generate_balanced_slots in Ambrilindo Local.
pub fn generate_balanced_slots(n: usize, rng: &mut impl Rng) -> Vec<MatchSlot> {
    let n = n.min(16);
    let mut slots = Vec::with_capacity(n);

    // 1. Primary slots (at most 8)
    let n_primary = n.min(8);
    let mut n_top = n_primary / 2;
    let mut n_bot = n_primary - n_top;
    if n_top != n_bot && rng.gen_bool(0.5) {
        std::mem::swap(&mut n_top, &mut n_bot);
    }

    let mut q1 = n_top / 2;
    let mut q2 = n_top - q1;
    if q1 != q2 && rng.gen_bool(0.5) {
        std::mem::swap(&mut q1, &mut q2);
    }

    let mut q3 = n_bot / 2;
    let mut q4 = n_bot - q3;
    if q3 != q4 && rng.gen_bool(0.5) {
        std::mem::swap(&mut q3, &mut q4);
    }

    let quadrants = [
        (Quadrant::Q1, q1),
        (Quadrant::Q2, q2),
        (Quadrant::Q3, q3),
        (Quadrant::Q4, q4),
    ];

    for (q, cnt) in quadrants {
        let (outer_m, inner_m) = quadrant_matches(q);
        if cnt == 1 {
            let chosen_m = if rng.gen_bool(0.5) { outer_m } else { inner_m };
            slots.push(MatchSlot {
                match_num: chosen_m,
                position: primary_slot(chosen_m),
            });
        } else if cnt == 2 {
            slots.push(MatchSlot {
                match_num: outer_m,
                position: primary_slot(outer_m),
            });
            slots.push(MatchSlot {
                match_num: inner_m,
                position: primary_slot(inner_m),
            });
        }
    }

    // 2. Secondary slots (when n > 8)
    let n_sec = if n > 8 { n - 8 } else { 0 };
    if n_sec > 0 {
        let mut sec_top = n_sec / 2;
        let mut sec_bot = n_sec - sec_top;
        if sec_top != sec_bot && rng.gen_bool(0.5) {
            std::mem::swap(&mut sec_top, &mut sec_bot);
        }

        let mut sq1 = sec_top / 2;
        let mut sq2 = sec_top - sq1;
        if sq1 != sq2 && rng.gen_bool(0.5) {
            std::mem::swap(&mut sq1, &mut sq2);
        }

        let mut sq3 = sec_bot / 2;
        let mut sq4 = sec_bot - sq3;
        if sq3 != sq4 && rng.gen_bool(0.5) {
            std::mem::swap(&mut sq3, &mut sq4);
        }

        let sec_quadrants = [
            (Quadrant::Q1, sq1),
            (Quadrant::Q2, sq2),
            (Quadrant::Q3, sq3),
            (Quadrant::Q4, sq4),
        ];

        for (q, cnt) in sec_quadrants {
            let (outer_m, inner_m) = quadrant_matches(q);
            if cnt == 1 {
                // Prioritize inner matches for preliminary fight so corner seeds retain BYEs
                let chosen_m = if rng.gen_bool(0.75) { inner_m } else { outer_m };
                slots.push(MatchSlot {
                    match_num: chosen_m,
                    position: secondary_slot(chosen_m),
                });
            } else if cnt == 2 {
                slots.push(MatchSlot {
                    match_num: inner_m,
                    position: secondary_slot(inner_m),
                });
                slots.push(MatchSlot {
                    match_num: outer_m,
                    position: secondary_slot(outer_m),
                });
            }
        }
    }

    slots
}

#[derive(Debug, Clone)]
pub struct AthleteInput {
    pub id: i64,
    pub name: String,
    pub is_priority: bool,
    pub group_key: String, // club or contingent identifier
}

#[derive(Debug, Clone, Default)]
pub struct MatchData {
    pub atlet1_id: Option<i64>,
    pub atlet2_id: Option<i64>,
    pub vr1: bool,
    pub vr2: bool,
}

#[derive(Debug, Clone)]
pub struct BracketResult {
    // round -> urutan -> MatchData
    // round 1: 1..=8, round 2: 1..=4, round 3: 1..=2, round 4: 1, round 5: 1
    pub matches: HashMap<u32, HashMap<u32, MatchData>>,
    pub collision_count: usize,
}

/// Distributes athletes across `num_pools` pools such that:
/// 1. Total athlete count in every pool differs by at most 1 (|pool_i| - |pool_j| <= 1).
/// 2. Priority athletes (is_priority=True) are distributed as evenly as possible across pools.
/// 3. Athletes from each delegation/club are split as evenly as possible across all pools.
/// 4. Remainder athletes are assigned to pools with greatest remaining capacity and randomized tie-breaking.
pub fn split_athletes_into_pools(
    athletes: &[AthleteInput],
    num_pools: usize,
) -> Vec<Vec<AthleteInput>> {
    let total_atlets = athletes.len();
    if num_pools <= 1 {
        return vec![athletes.to_vec()];
    }

    let mut rng = rand::thread_rng();
    let base = total_atlets / num_pools;
    let remainder = total_atlets % num_pools;
    let mut target_counts = vec![base; num_pools];

    let mut rem_pools: Vec<usize> = (0..num_pools).collect();
    rem_pools.shuffle(&mut rng);
    for &p in rem_pools.iter().take(remainder) {
        target_counts[p] += 1;
    }

    let priority_atlets: Vec<AthleteInput> = athletes.iter().filter(|a| a.is_priority).cloned().collect();
    let regular_atlets: Vec<AthleteInput> = athletes.iter().filter(|a| !a.is_priority).cloned().collect();

    let mut pools_atlets: Vec<Vec<AthleteInput>> = vec![Vec::new(); num_pools];

    // 1. Distribute priority athletes across pools first
    if !priority_atlets.is_empty() {
        let mut p_delegations: HashMap<String, Vec<AthleteInput>> = HashMap::new();
        for a in priority_atlets {
            p_delegations.entry(a.group_key.clone()).or_default().push(a);
        }

        let mut p_groups: Vec<Vec<AthleteInput>> = p_delegations.into_values().collect();
        p_groups.shuffle(&mut rng);
        p_groups.sort_by_key(|g| std::cmp::Reverse(g.len()));

        for mut p_list in p_groups {
            p_list.shuffle(&mut rng);
            for a in p_list {
                let mut eligible_pools: Vec<usize> = (0..num_pools)
                    .filter(|&p| pools_atlets[p].len() < target_counts[p])
                    .collect();
                if eligible_pools.is_empty() {
                    eligible_pools = (0..num_pools).collect();
                }

                // Sort by: fewest priority athletes, fewest total athletes, random tiebreak
                eligible_pools.sort_by(|&p1, &p2| {
                    let prio1 = pools_atlets[p1].iter().filter(|x| x.is_priority).count();
                    let prio2 = pools_atlets[p2].iter().filter(|x| x.is_priority).count();
                    if prio1 != prio2 {
                        return prio1.cmp(&prio2);
                    }
                    let len1 = pools_atlets[p1].len();
                    let len2 = pools_atlets[p2].len();
                    if len1 != len2 {
                        return len1.cmp(&len2);
                    }
                    rng.gen_range(0..2).cmp(&1)
                });

                pools_atlets[eligible_pools[0]].push(a);
            }
        }
    }

    // 2. Distribute regular athletes across pools balancing delegations
    if !regular_atlets.is_empty() {
        let mut delegations: HashMap<String, Vec<AthleteInput>> = HashMap::new();
        for a in regular_atlets {
            delegations.entry(a.group_key.clone()).or_default().push(a);
        }

        for list in delegations.values_mut() {
            list.shuffle(&mut rng);
        }

        let mut sorted_groups: Vec<(String, Vec<AthleteInput>)> = delegations.into_iter().collect();
        sorted_groups.sort_by_key(|(_, list)| std::cmp::Reverse(list.len()));

        for (_gid, a_list) in sorted_groups {
            let k = a_list.len();
            let base_give = k / num_pools;

            let mut curr = 0;
            if base_give > 0 {
                for p in 0..num_pools {
                    let space = target_counts[p].saturating_sub(pools_atlets[p].len());
                    let to_add = base_give.min(space);
                    if to_add > 0 {
                        pools_atlets[p].extend(a_list[curr..curr + to_add].iter().cloned());
                        curr += to_add;
                    }
                }
            }

            let remaining_athletes = &a_list[curr..];
            for a in remaining_athletes {
                let mut pool_capacities: Vec<(usize, isize, f64, usize)> = Vec::new();
                for p in 0..num_pools {
                    let cap = target_counts[p] as isize - pools_atlets[p].len() as isize;
                    if cap > 0 {
                        let same_del = pools_atlets[p].iter().filter(|x| x.group_key == a.group_key).count();
                        pool_capacities.push((same_del, -cap, rng.gen_range(0.0..1.0), p));
                    }
                }

                if !pool_capacities.is_empty() {
                    pool_capacities.sort_by(|x, y| {
                        x.0.cmp(&y.0)
                            .then_with(|| x.1.cmp(&y.1))
                            .then_with(|| x.2.partial_cmp(&y.2).unwrap_or(std::cmp::Ordering::Equal))
                    });
                    let best_p = pool_capacities[0].3;
                    pools_atlets[best_p].push(a.clone());
                } else {
                    let p_min = (0..num_pools).min_by_key(|&p| pools_atlets[p].len()).unwrap_or(0);
                    pools_atlets[p_min].push(a.clone());
                }
            }
        }
    }

    pools_atlets
}

/// Builds the complete 5-round tournament bracket in memory.
/// Replicates legacy build_bracket_in_memory:
/// - 60 Monte Carlo trials searching for optimal anti-collision placement
/// - Priority athletes receive highest BYE levels and opposite half distribution
/// - Subtree-aware automatic BYE advancement across Rounds 1 -> 2 -> 3 -> 4
/// - Pre-seeds Video Review (VR) on Semifinals & Finals
pub fn build_bracket_in_memory(
    athletes: &[AthleteInput],
    pool: i32,
    has_vr: bool,
) -> BracketResult {
    let mut matches: HashMap<u32, HashMap<u32, MatchData>> = HashMap::new();
    for r in 1..=5 {
        let max_u = match r {
            1 => 8,
            2 => 4,
            3 => 2,
            4 => 1,
            5 => 1,
            _ => 1,
        };
        let mut r_map = HashMap::new();
        for u in 1..=max_u {
            r_map.insert(u, MatchData::default());
        }
        matches.insert(r, r_map);
    }

    if athletes.is_empty() {
        if has_vr {
            for u in 1..=2 {
                if let Some(m) = matches.get_mut(&3).and_then(|r| r.get_mut(&u)) {
                    m.vr1 = true;
                    m.vr2 = true;
                }
            }
            if let Some(m) = matches.get_mut(&4).and_then(|r| r.get_mut(&1)) {
                m.vr1 = true;
                m.vr2 = true;
            }
            if pool == 0 {
                if let Some(m) = matches.get_mut(&1).and_then(|r| r.get_mut(&1)) {
                    m.vr1 = true;
                    m.vr2 = true;
                }
            }
        }
        return BracketResult {
            matches,
            collision_count: 0,
        };
    }

    let athletes_slice = &athletes[..athletes.len().min(16)];
    let priority_athletes: Vec<AthleteInput> = athletes_slice.iter().filter(|a| a.is_priority).cloned().collect();
    let regular_athletes: Vec<AthleteInput> = athletes_slice.iter().filter(|a| !a.is_priority).cloned().collect();

    let mut rng = rand::thread_rng();
    let num_trials = 60;
    let mut best_score = u64::MAX;
    let mut best_candidates: Vec<HashMap<i64, MatchSlot>> = Vec::new();

    for _ in 0..num_trials {
        let mut slots = generate_balanced_slots(athletes_slice.len(), &mut rng);
        slots.shuffle(&mut rng);
        let mut available = slots.clone();
        let mut assignment: HashMap<i64, MatchSlot> = HashMap::new();
        let mut placed_by_group: HashMap<String, Vec<MatchSlot>> = HashMap::new();
        let mut placed_priority_slots: Vec<MatchSlot> = Vec::new();
        let mut trial_score: u64 = 0;

        let mut match_counts: HashMap<u32, usize> = HashMap::new();
        for s in &slots {
            *match_counts.entry(s.match_num).or_default() += 1;
        }

        let get_slot_bye_level = |s: &MatchSlot, counts: &HashMap<u32, usize>| -> u32 {
            let m = s.match_num;
            if counts.get(&m).copied().unwrap_or(0) > 1 {
                return 0; // No BYE (Round 1 fight)
            }
            let paired_m = if m % 2 != 0 { m + 1 } else { m - 1 };
            if counts.get(&paired_m).copied().unwrap_or(0) > 0 {
                return 1; // Single BYE (advances to Quarterfinal / Round 2)
            }
            let half = half_of_match(m);
            let opposing_quad_matches: [u32; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
            let opp_count: usize = opposing_quad_matches
                .iter()
                .filter(|&&om| half_of_match(om) == half && quadrant_of_match(om) != quadrant_of_match(m))
                .map(|&om| counts.get(&om).copied().unwrap_or(0))
                .sum();
            if opp_count == 0 {
                3 // Triple BYE (advances to Final / Round 4)
            } else {
                2 // Double BYE (advances to Semifinal / Round 3)
            }
        };

        // 1. Place priority athletes into highest BYE level slots first
        if !priority_athletes.is_empty() {
            let mut shuffled_prio = priority_athletes.clone();
            shuffled_prio.shuffle(&mut rng);

            for a in shuffled_prio {
                let max_bye = available.iter().map(|s| get_slot_bye_level(s, &match_counts)).max().unwrap_or(0);
                let candidate_slots: Vec<MatchSlot> = available
                    .iter()
                    .filter(|s| get_slot_bye_level(s, &match_counts) == max_bye)
                    .cloned()
                    .collect();

                let mut best_p = u64::MAX;
                let mut candidates: Vec<MatchSlot> = Vec::new();
                for s in &candidate_slots {
                    let p_prio: u64 = placed_priority_slots.iter().map(|prev_s| slot_penalty(s, prev_s)).sum();
                    let empty_group = Vec::new();
                    let group_prev = placed_by_group.get(&a.group_key).unwrap_or(&empty_group);
                    let p_del: u64 = group_prev.iter().map(|prev_s| slot_penalty(s, prev_s)).sum();
                    let p = 2 * p_prio + p_del;
                    if p < best_p {
                        best_p = p;
                        candidates = vec![*s];
                    } else if p == best_p {
                        candidates.push(*s);
                    }
                }

                if candidates.is_empty() {
                    candidates = available.clone();
                    best_p = 0;
                }

                let chosen = *candidates.choose(&mut rng).unwrap();
                if let Some(pos) = available.iter().position(|x| *x == chosen) {
                    available.remove(pos);
                }
                assignment.insert(a.id, chosen);
                placed_priority_slots.push(chosen);
                placed_by_group.entry(a.group_key.clone()).or_default().push(chosen);
                trial_score = trial_score.saturating_add(best_p);
                if max_bye == 0 {
                    trial_score = trial_score.saturating_add(500_000);
                }
            }
        }

        // 2. Place regular athletes balancing delegations
        if !regular_athletes.is_empty() {
            let mut reg_delegations: HashMap<String, Vec<AthleteInput>> = HashMap::new();
            for a in regular_athletes.clone() {
                reg_delegations.entry(a.group_key.clone()).or_default().push(a);
            }

            let mut groups: Vec<(String, Vec<AthleteInput>)> = reg_delegations.into_iter().collect();
            groups.shuffle(&mut rng);
            groups.sort_by_key(|(_, list)| std::cmp::Reverse(list.len()));

            for (gid, a_list) in groups {
                let mut shuffled_a = a_list;
                shuffled_a.shuffle(&mut rng);

                for a in shuffled_a {
                    let mut best_p = u64::MAX;
                    let mut candidates: Vec<MatchSlot> = Vec::new();

                    for s in &available {
                        let empty_group = Vec::new();
                        let group_prev = placed_by_group.get(&gid).unwrap_or(&empty_group);
                        let p: u64 = group_prev.iter().map(|prev_s| slot_penalty(s, prev_s)).sum();
                        if p < best_p {
                            best_p = p;
                            candidates = vec![*s];
                        } else if p == best_p {
                            candidates.push(*s);
                        }
                    }

                    if candidates.is_empty() {
                        candidates = available.clone();
                        best_p = 0;
                    }

                    let chosen = *candidates.choose(&mut rng).unwrap();
                    if let Some(pos) = available.iter().position(|x| *x == chosen) {
                        available.remove(pos);
                    }
                    assignment.insert(a.id, chosen);
                    placed_by_group.entry(gid.clone()).or_default().push(chosen);
                    trial_score = trial_score.saturating_add(best_p);
                }
            }
        }

        if trial_score < best_score {
            best_score = trial_score;
            best_candidates = vec![assignment];
        } else if trial_score == best_score {
            best_candidates.push(assignment);
        }
    }

    let best_assignment = best_candidates.choose(&mut rng).cloned().unwrap_or_default();

    // Populate Round 1 matches
    let mut initial_round_1: HashMap<(u32, SlotPosition), i64> = HashMap::new();
    let athlete_map: HashMap<i64, &AthleteInput> = athletes_slice.iter().map(|a| (a.id, a)).collect();

    for (&a_id, slot) in &best_assignment {
        if let Some(r1_map) = matches.get_mut(&1) {
            if let Some(match_slot) = r1_map.get_mut(&slot.match_num) {
                match slot.position {
                    SlotPosition::Atlet1 => {
                        match_slot.atlet1_id = Some(a_id);
                        if has_vr {
                            match_slot.vr1 = true;
                        }
                    }
                    SlotPosition::Atlet2 => {
                        match_slot.atlet2_id = Some(a_id);
                        if has_vr {
                            match_slot.vr2 = true;
                        }
                    }
                }
            }
        }
        initial_round_1.insert((slot.match_num, slot.position), a_id);
    }

    // Count Round 1 collisions
    let mut collision_count = 0;
    if let Some(r1_map) = matches.get(&1) {
        for u in 1..=8 {
            if let Some(m) = r1_map.get(&u) {
                if let (Some(id1), Some(id2)) = (m.atlet1_id, m.atlet2_id) {
                    if let (Some(a1), Some(a2)) = (athlete_map.get(&id1), athlete_map.get(&id2)) {
                        if a1.group_key == a2.group_key {
                            collision_count += 1;
                        }
                    }
                }
            }
        }
    }

    // Subtree-aware BYE advancement
    let count_initial = |slots_to_check: &[(u32, SlotPosition)]| -> usize {
        slots_to_check.iter().filter(|s| initial_round_1.contains_key(s)).count()
    };

    // Round 1 -> 2
    for u in 1..=8 {
        let target_u = (u + 1) / 2;
        let target_is_atlet1 = u % 2 != 0;

        let m1 = matches.get(&1).and_then(|r| r.get(&u)).cloned().unwrap_or_default();
        if m1.atlet1_id.is_some() && m1.atlet2_id.is_none() {
            let athlete_id = m1.atlet1_id;
            if let Some(r2_map) = matches.get_mut(&2) {
                if let Some(m2) = r2_map.get_mut(&target_u) {
                    if target_is_atlet1 {
                        m2.atlet1_id = athlete_id;
                        if has_vr { m2.vr1 = true; }
                    } else {
                        m2.atlet2_id = athlete_id;
                        if has_vr { m2.vr2 = true; }
                    }
                }
            }
            if let Some(r1_map) = matches.get_mut(&1) {
                if let Some(m1_mut) = r1_map.get_mut(&u) {
                    m1_mut.atlet1_id = None;
                    m1_mut.vr1 = false;
                }
            }
        } else if m1.atlet2_id.is_some() && m1.atlet1_id.is_none() {
            let athlete_id = m1.atlet2_id;
            if let Some(r2_map) = matches.get_mut(&2) {
                if let Some(m2) = r2_map.get_mut(&target_u) {
                    if target_is_atlet1 {
                        m2.atlet1_id = athlete_id;
                        if has_vr { m2.vr1 = true; }
                    } else {
                        m2.atlet2_id = athlete_id;
                        if has_vr { m2.vr2 = true; }
                    }
                }
            }
            if let Some(r1_map) = matches.get_mut(&1) {
                if let Some(m1_mut) = r1_map.get_mut(&u) {
                    m1_mut.atlet2_id = None;
                    m1_mut.vr2 = false;
                }
            }
        }
    }

    // Pre-seed Semi-Finals (Round 3) if has_vr
    if has_vr {
        if let Some(r3_map) = matches.get_mut(&3) {
            for target_u in 1..=2 {
                if let Some(m3) = r3_map.get_mut(&target_u) {
                    m3.vr1 = true;
                    m3.vr2 = true;
                }
            }
        }
    }

    // Round 2 -> 3
    for u in 1..=4 {
        let target_u = (u + 1) / 2;
        let target_is_atlet1 = u % 2 != 0;

        let m2 = matches.get(&2).and_then(|r| r.get(&u)).cloned().unwrap_or_default();

        if let Some(a1_id) = m2.atlet1_id {
            let opposing_slots = [(2 * u, SlotPosition::Atlet1), (2 * u, SlotPosition::Atlet2)];
            if count_initial(&opposing_slots) == 0 {
                if let Some(r3_map) = matches.get_mut(&3) {
                    if let Some(m3) = r3_map.get_mut(&target_u) {
                        if target_is_atlet1 {
                            m3.atlet1_id = Some(a1_id);
                            if has_vr { m3.vr1 = true; }
                        } else {
                            m3.atlet2_id = Some(a1_id);
                            if has_vr { m3.vr2 = true; }
                        }
                    }
                }
                if let Some(r2_map) = matches.get_mut(&2) {
                    if let Some(m2_mut) = r2_map.get_mut(&u) {
                        m2_mut.atlet1_id = None;
                        m2_mut.vr1 = false;
                    }
                }
            }
        }

        if let Some(a2_id) = m2.atlet2_id {
            let opposing_slots = [(2 * u - 1, SlotPosition::Atlet1), (2 * u - 1, SlotPosition::Atlet2)];
            if count_initial(&opposing_slots) == 0 {
                if let Some(r3_map) = matches.get_mut(&3) {
                    if let Some(m3) = r3_map.get_mut(&target_u) {
                        if target_is_atlet1 {
                            m3.atlet1_id = Some(a2_id);
                            if has_vr { m3.vr1 = true; }
                        } else {
                            m3.atlet2_id = Some(a2_id);
                            if has_vr { m3.vr2 = true; }
                        }
                    }
                }
                if let Some(r2_map) = matches.get_mut(&2) {
                    if let Some(m2_mut) = r2_map.get_mut(&u) {
                        m2_mut.atlet2_id = None;
                        m2_mut.vr2 = false;
                    }
                }
            }
        }
    }

    // Pre-seed Finals (Round 4) if has_vr
    if has_vr {
        if let Some(r4_map) = matches.get_mut(&4) {
            if let Some(m4) = r4_map.get_mut(&1) {
                m4.vr1 = true;
                m4.vr2 = true;
            }
        }
    }

    // Round 3 -> 4
    let q_slots = |q: Quadrant| -> Vec<(u32, SlotPosition)> {
        let (outer, inner) = quadrant_matches(q);
        vec![
            (outer, SlotPosition::Atlet1),
            (outer, SlotPosition::Atlet2),
            (inner, SlotPosition::Atlet1),
            (inner, SlotPosition::Atlet2),
        ]
    };

    let r3_opposing: [((u32, SlotPosition), Vec<(u32, SlotPosition)>); 4] = [
        ((1, SlotPosition::Atlet1), q_slots(Quadrant::Q2)),
        ((1, SlotPosition::Atlet2), q_slots(Quadrant::Q1)),
        ((2, SlotPosition::Atlet1), q_slots(Quadrant::Q4)),
        ((2, SlotPosition::Atlet2), q_slots(Quadrant::Q3)),
    ];

    for u in 1..=2 {
        let target_is_atlet1 = u == 1;
        let m3 = matches.get(&3).and_then(|r| r.get(&u)).cloned().unwrap_or_default();

        if let Some(a1_id) = m3.atlet1_id {
            if let Some((_, opp)) = r3_opposing.iter().find(|(k, _)| *k == (u, SlotPosition::Atlet1)) {
                if count_initial(opp) == 0 {
                    if let Some(r4_map) = matches.get_mut(&4) {
                        if let Some(m4) = r4_map.get_mut(&1) {
                            if target_is_atlet1 {
                                m4.atlet1_id = Some(a1_id);
                                if has_vr { m4.vr1 = true; }
                            } else {
                                m4.atlet2_id = Some(a1_id);
                                if has_vr { m4.vr2 = true; }
                            }
                        }
                    }
                    if let Some(r3_map) = matches.get_mut(&3) {
                        if let Some(m3_mut) = r3_map.get_mut(&u) {
                            m3_mut.atlet1_id = None;
                            m3_mut.vr1 = false;
                        }
                    }
                }
            }
        }

        if let Some(a2_id) = m3.atlet2_id {
            if let Some((_, opp)) = r3_opposing.iter().find(|(k, _)| *k == (u, SlotPosition::Atlet2)) {
                if count_initial(opp) == 0 {
                    if let Some(r4_map) = matches.get_mut(&4) {
                        if let Some(m4) = r4_map.get_mut(&1) {
                            if target_is_atlet1 {
                                m4.atlet1_id = Some(a2_id);
                                if has_vr { m4.vr1 = true; }
                            } else {
                                m4.atlet2_id = Some(a2_id);
                                if has_vr { m4.vr2 = true; }
                            }
                        }
                    }
                    if let Some(r3_map) = matches.get_mut(&3) {
                        if let Some(m3_mut) = r3_map.get_mut(&u) {
                            m3_mut.atlet2_id = None;
                            m3_mut.vr2 = false;
                        }
                    }
                }
            }
        }
    }

    BracketResult {
        matches,
        collision_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slot_distance_and_penalties() {
        let s1 = MatchSlot { match_num: 1, position: SlotPosition::Atlet1 };
        let s2 = MatchSlot { match_num: 1, position: SlotPosition::Atlet2 };
        let s3 = MatchSlot { match_num: 2, position: SlotPosition::Atlet1 };
        let s4 = MatchSlot { match_num: 3, position: SlotPosition::Atlet1 };
        let s5 = MatchSlot { match_num: 5, position: SlotPosition::Atlet1 };

        assert_eq!(slot_distance(&s1, &s2), 1);
        assert_eq!(slot_penalty(&s1, &s2), 1_000_000);

        assert_eq!(slot_distance(&s1, &s3), 2);
        assert_eq!(slot_penalty(&s1, &s3), 10_000);

        assert_eq!(slot_distance(&s1, &s4), 3);
        assert_eq!(slot_penalty(&s1, &s4), 100);

        assert_eq!(slot_distance(&s1, &s5), 4);
        assert_eq!(slot_penalty(&s1, &s5), 0);
    }

    #[test]
    fn test_balanced_slots_count() {
        let mut rng = rand::thread_rng();
        for n in 1..=16 {
            let slots = generate_balanced_slots(n, &mut rng);
            assert_eq!(slots.len(), n, "Should generate exactly n slots for n={}", n);
        }
    }

    #[test]
    fn test_split_athletes_into_pools_parity() {
        let athletes: Vec<AthleteInput> = (1..=17)
            .map(|i| AthleteInput {
                id: i,
                name: format!("Athlete {}", i),
                is_priority: i <= 2,
                group_key: format!("Club {}", i % 3),
            })
            .collect();

        let pools = split_athletes_into_pools(&athletes, 2);
        assert_eq!(pools.len(), 2);
        assert_eq!(pools[0].len() + pools[1].len(), 17);
        let diff = (pools[0].len() as isize - pools[1].len() as isize).abs();
        assert!(diff <= 1, "Pool athlete counts must differ by at most 1, got diff={}", diff);

        // Verify priority athletes are distributed across both pools
        let prio0 = pools[0].iter().filter(|a| a.is_priority).count();
        let prio1 = pools[1].iter().filter(|a| a.is_priority).count();
        assert_eq!(prio0 + prio1, 2);
        assert_eq!(prio0, 1);
        assert_eq!(prio1, 1);
    }

    #[test]
    fn test_split_athletes_into_four_pools() {
        let athletes: Vec<AthleteInput> = (1..=33)
            .map(|i| AthleteInput {
                id: i,
                name: format!("Athlete {}", i),
                is_priority: false,
                group_key: format!("Club {}", i % 4),
            })
            .collect();

        let pools = split_athletes_into_pools(&athletes, 4);
        assert_eq!(pools.len(), 4);
        let total: usize = pools.iter().map(|p| p.len()).sum();
        assert_eq!(total, 33);
        for i in 0..4 {
            for j in i + 1..4 {
                let diff = (pools[i].len() as isize - pools[j].len() as isize).abs();
                assert!(diff <= 1, "Pool counts {} and {} diff must be <= 1", pools[i].len(), pools[j].len());
            }
        }
    }

    #[test]
    fn test_build_bracket_with_single_athlete_advances_to_final() {
        let athletes = vec![AthleteInput {
            id: 101,
            name: "Sole Competitor".into(),
            is_priority: true,
            group_key: "Solo".into(),
        }];

        let result = build_bracket_in_memory(&athletes, 1, false);
        assert_eq!(result.collision_count, 0);

        // A single competitor has empty branches everywhere and should advance to Round 4 (Final)
        let r4_match = result.matches.get(&4).unwrap().get(&1).unwrap();
        assert!(
            r4_match.atlet1_id == Some(101) || r4_match.atlet2_id == Some(101),
            "Sole competitor should advance all the way to Round 4 (Final)"
        );
    }

    #[test]
    fn test_build_bracket_two_teammates_opposite_halves() {
        let athletes = vec![
            AthleteInput { id: 1, name: "Teammate 1".into(), is_priority: false, group_key: "KlubA".into() },
            AthleteInput { id: 2, name: "Teammate 2".into(), is_priority: false, group_key: "KlubA".into() },
        ];

        let result = build_bracket_in_memory(&athletes, 1, false);
        assert_eq!(result.collision_count, 0);

        // Both should advance and not meet in Round 1
        let r1_matches = result.matches.get(&1).unwrap();
        for (_, m) in r1_matches {
            assert!(!(m.atlet1_id.is_some() && m.atlet2_id.is_some()), "Teammates must not face each other in Round 1");
        }
    }
}
