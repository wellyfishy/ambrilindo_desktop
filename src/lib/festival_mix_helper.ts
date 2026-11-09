/**
 * Festival Category Mix Recommendation Engine & Parser
 *
 * Implements standard karate competition mixing principles:
 * 1. Strict Discipline Isolation: KATA only with KATA, KUMITE only with KUMITE.
 * 2. Strict Gender Isolation: PUTRA only with PUTRA, PUTRI only with PUTRI.
 * 3. Proportional Weight Matching (Kumite):
 *    - Best match: Complementary brackets (-55kg and +55kg) or adjacent weights within <= 5kg.
 *    - Fair match: Difference within 6-10kg (with note).
 *    - Incompatible: Difference > 10kg is disallowed for athlete safety.
 * 4. Age Tier Proximity:
 *    - Same Age Group: Best Match (Tier difference = 0, no warning).
 *    - Adjacent Age Group: Allowed with Warning (Tier difference = 1, e.g. Pra Usia Dini -> Usia Dini).
 *    - Tier difference >= 2: Disallowed (e.g. Pra Usia Dini -> Pemula).
 * 5. Multi-Class (Tri-Category) Mix Support:
 *    - Combines 2, 3, or more categories into 1 festival bagan sheet (up to 6 groups / 24 athletes).
 */

export interface ParsedFestivalCategory {
  rawName: string;
  discipline: "kata" | "kumite" | "unknown";
  gender: "putra" | "putri" | "unknown";
  ageGroup: string | null;
  ageTier: number; // 1 to 7
  weightSign: "+" | "-" | null;
  weightVal: number | null; // e.g. 20, 25, 30, 35, 40, 45, 50, 55
}

export type MixMatchQuality = "best" | "warning" | "incompatible";

export interface MixCandidateRecommendation {
  categoryId: number;
  categoryName: string;
  athleteCount: number;
  quality: MixMatchQuality;
  score: number; // Higher is better (0 - 100)
  reasons: string[];
  warnings: string[];
  suggestedName: string;
  parsed: ParsedFestivalCategory;
}

export interface FestivalCategoryItemLike {
  id: number;
  nama: string;
  athlete_count?: number;
  atlet_count?: number;
}

export const AGE_TIER_DEFINITIONS: { name: string; tier: number; label: string }[] = [
  { name: "PRA USIA DINI", tier: 1, label: "Pra Usia Dini" },
  { name: "USIA DINI", tier: 2, label: "Usia Dini" },
  { name: "PRA PEMULA", tier: 3, label: "Pra Pemula" },
  { name: "PEMULA", tier: 4, label: "Pemula" },
  { name: "KADET", tier: 5, label: "Kadet" },
  { name: "CADET", tier: 5, label: "Kadet" },
  { name: "JUNIOR", tier: 6, label: "Junior" },
  { name: "UNDER 21", tier: 7, label: "Under 21" },
  { name: "SENIOR", tier: 7, label: "Senior" },
];

/**
 * Parses raw category name into structured karate competition attributes.
 */
export function parseFestivalCategory(name: string): ParsedFestivalCategory {
  const u = name.toUpperCase();

  // 1. Discipline
  const discipline: "kata" | "kumite" | "unknown" = u.includes("KUMITE")
    ? "kumite"
    : u.includes("KATA")
    ? "kata"
    : "unknown";

  // 2. Gender
  const gender: "putri" | "putra" | "unknown" = u.includes("PUTRI")
    ? "putri"
    : u.includes("PUTRA")
    ? "putra"
    : "unknown";

  // 3. Age Tier
  let ageGroup: string | null = null;
  let ageTier = 0;
  for (const def of AGE_TIER_DEFINITIONS) {
    if (u.includes(def.name)) {
      ageGroup = def.label;
      ageTier = def.tier;
      break;
    }
  }

  // 4. Weight (for Kumite)
  // Matches patterns like "-25 KG", "+55KG", "-30", "+45"
  const weightMatch = u.match(/([+-])\s*(\d+)\s*(?:KG)?/);
  const weightSign = weightMatch ? (weightMatch[1] as "+" | "-") : null;
  const weightVal = weightMatch ? parseInt(weightMatch[2], 10) : null;

  return {
    rawName: name,
    discipline,
    gender,
    ageGroup,
    ageTier,
    weightSign,
    weightVal,
  };
}

/**
 * Helper to join list of strings into natural Indonesian format:
 * ["A"] => "A"
 * ["A", "B"] => "A & B"
 * ["A", "B", "C"] => "A, B & C"
 */
export function formatListWithAnd(items: string[]): string {
  if (!items || items.length === 0) return "";
  if (items.length === 1) return items[0];
  if (items.length === 2) return `${items[0]} & ${items[1]}`;
  return `${items.slice(0, -1).join(", ")} & ${items[items.length - 1]}`;
}

/**
 * Generates an appropriate standardized merged category name for 2 or more categories.
 * e.g.:
 * - 2 classes same age: "FESTIVAL PEMULA - KUMITE (-55 KG & +55 KG / OPEN) PUTRA"
 * - 2 classes different age: "FESTIVAL (MIX PRA USIA DINI & USIA DINI) - KATA PERORANGAN PUTRI"
 * - 3 classes different age: "FESTIVAL (MIX PRA USIA DINI, USIA DINI & PRA PEMULA) - KATA PERORANGAN PUTRI"
 * - 3 classes kumite: "FESTIVAL (MIX PRA PEMULA & PEMULA) - KUMITE (MIX -35 KG, +35 KG & -45 KG) PUTRA"
 */
export function suggestMultiCategoryBaganName(
  categories: (FestivalCategoryItemLike | ParsedFestivalCategory | string)[]
): string {
  if (!categories || categories.length === 0) return "FESTIVAL BAGAN MIX";

  const parsedList: ParsedFestivalCategory[] = categories.map((c) => {
    if (typeof c === "string") return parseFestivalCategory(c);
    if ("discipline" in c) return c as ParsedFestivalCategory;
    return parseFestivalCategory(c.nama);
  });

  const first = parsedList[0];
  const discStr = first.discipline === "kumite" ? "KUMITE" : "KATA PERORANGAN";
  const genderStr = first.gender === "putri" ? "PUTRI" : "PUTRA";

  // Sort by age tier
  const sortedByTier = [...parsedList].sort((a, b) => a.ageTier - b.ageTier);

  // Collect unique age groups in ascending tier order
  const seenAges: string[] = [];
  for (const p of sortedByTier) {
    if (p.ageGroup && !seenAges.includes(p.ageGroup.toUpperCase())) {
      seenAges.push(p.ageGroup.toUpperCase());
    }
  }

  // Check unique tiers
  const uniqueTiers = new Set(parsedList.map((p) => p.ageTier).filter((t) => t > 0));

  // Collect unique weights for Kumite
  const weights: string[] = [];
  for (const p of parsedList) {
    if (p.weightVal !== null) {
      const wStr = `${p.weightSign || ""}${p.weightVal} KG`;
      if (!weights.includes(wStr)) {
        weights.push(wStr);
      }
    }
  }

  // Case 1: Same Age Group (or only 1 recognized age group)
  if (uniqueTiers.size <= 1) {
    const ageStr = seenAges.length > 0 ? seenAges[0] : "FESTIVAL";

    if (first.discipline === "kumite") {
      // Complementary pair (e.g. -55 and +55)
      if (
        parsedList.length === 2 &&
        parsedList[0].weightVal !== null &&
        parsedList[0].weightVal === parsedList[1].weightVal &&
        parsedList[0].weightSign !== parsedList[1].weightSign
      ) {
        return `FESTIVAL ${ageStr} - KUMITE (-${parsedList[0].weightVal} KG & +${parsedList[0].weightVal} KG / OPEN) ${genderStr}`;
      }
      if (weights.length > 0) {
        return `FESTIVAL ${ageStr} - KUMITE (MIX ${formatListWithAnd(weights)}) ${genderStr}`;
      }
      return `FESTIVAL ${ageStr} - KUMITE (MIX OPEN) ${genderStr}`;
    }

    return `FESTIVAL ${ageStr} - ${discStr} ${genderStr}`;
  }

  // Case 2: Multiple Age Groups (Mix 2, 3 or more age tiers)
  const mixAgeHeader = `FESTIVAL (MIX ${formatListWithAnd(seenAges)})`;

  if (first.discipline === "kumite") {
    if (weights.length > 0) {
      return `${mixAgeHeader} - KUMITE (MIX ${formatListWithAnd(weights)}) ${genderStr}`;
    }
    return `${mixAgeHeader} - KUMITE ${genderStr}`;
  }

  return `${mixAgeHeader} - ${discStr} ${genderStr}`;
}

/**
 * Backward-compatible helper for 2 categories.
 */
export function suggestCombinedCategoryName(
  source: ParsedFestivalCategory | FestivalCategoryItemLike,
  target: ParsedFestivalCategory | FestivalCategoryItemLike
): string {
  return suggestMultiCategoryBaganName([source, target]);
}

/**
 * Evaluates candidate categories and returns sorted, scored recommendations.
 */
export function getFestivalMixRecommendations(
  sourceCategory: FestivalCategoryItemLike,
  allCandidates: FestivalCategoryItemLike[]
): MixCandidateRecommendation[] {
  const sourceParsed = parseFestivalCategory(sourceCategory.nama);
  const recommendations: MixCandidateRecommendation[] = [];

  for (const candidate of allCandidates) {
    if (candidate.id === sourceCategory.id) continue;

    const candidateParsed = parseFestivalCategory(candidate.nama);

    // Hard Rules 1 & 2: Discipline and Gender MUST match exactly
    if (
      sourceParsed.discipline !== "unknown" &&
      candidateParsed.discipline !== "unknown" &&
      sourceParsed.discipline !== candidateParsed.discipline
    ) {
      continue;
    }

    if (
      sourceParsed.gender !== "unknown" &&
      candidateParsed.gender !== "unknown" &&
      sourceParsed.gender !== candidateParsed.gender
    ) {
      continue;
    }

    // Hard Rule 3: Age tier difference must be <= 1
    let tierDiff = 0;
    if (sourceParsed.ageTier > 0 && candidateParsed.ageTier > 0) {
      tierDiff = Math.abs(sourceParsed.ageTier - candidateParsed.ageTier);
      if (tierDiff > 1) {
        // Disallowed: e.g. Pra Usia Dini with Pemula
        continue;
      }
    }

    // Scoring & Evaluation
    let score = 100;
    const reasons: string[] = [];
    const warnings: string[] = [];

    // Discipline & Gender affirmation
    if (sourceParsed.discipline !== "unknown") {
      reasons.push(
        sourceParsed.discipline === "kumite" ? "Sesama Kumite" : "Sesama Kata Perorangan"
      );
    }
    if (sourceParsed.gender !== "unknown") {
      reasons.push(sourceParsed.gender === "putri" ? "Sesama Putri" : "Sesama Putra");
    }

    // Age evaluation
    if (tierDiff === 0 && sourceParsed.ageGroup) {
      reasons.push(`Satu kelompok umur (${sourceParsed.ageGroup})`);
      score += 10;
    } else if (tierDiff === 1) {
      score -= 30;
      warnings.push(
        `⚠️ Peringatan Usia: Beda 1 kelompok umur (${sourceParsed.ageGroup || "Asal"} → ${candidateParsed.ageGroup || "Tujuan"})`
      );
    }

    // Weight evaluation (for Kumite)
    if (sourceParsed.discipline === "kumite") {
      if (sourceParsed.weightVal !== null && candidateParsed.weightVal !== null) {
        const weightDelta = Math.abs(sourceParsed.weightVal - candidateParsed.weightVal);

        if (
          sourceParsed.weightVal === candidateParsed.weightVal &&
          sourceParsed.weightSign !== candidateParsed.weightSign
        ) {
          // Exact complementary pair (e.g. -55 and +55)
          reasons.push(`Bobot berpasangan (-${sourceParsed.weightVal} & +${sourceParsed.weightVal} kg)`);
          score += 20;
        } else if (weightDelta <= 5) {
          reasons.push(`Selisih berat aman (hanya ${weightDelta} kg)`);
          score += 10;
        } else if (weightDelta <= 10) {
          score -= 25;
          warnings.push(`⚠️ Selisih berat badan cukup besar (${weightDelta} kg)`);
        } else {
          // Weight difference > 10kg is unsafe for festival children
          continue;
        }
      }
    }

    // Candidate athlete count bonus:
    const candAthletes = candidate.athlete_count ?? candidate.atlet_count ?? 0;
    if (candAthletes === 1) {
      reasons.push("Target sesama atlet solo (1 vs 1 ideal)");
      score += 30;
    } else if (candAthletes >= 2 && candAthletes <= 3) {
      reasons.push(`Target memiliki ${candAthletes} atlet (ideal membentuk pool 4)`);
      score += 15;
    } else if (candAthletes >= 4 && candAthletes <= 6) {
      reasons.push(`Target memiliki ${candAthletes} atlet (membentuk 2 grup)`);
      score += 5;
    } else if (candAthletes >= 7 && candAthletes <= 12) {
      reasons.push(`Target memiliki ${candAthletes} atlet (membentuk 3 grup)`);
    }

    const quality: MixMatchQuality = warnings.length > 0 ? "warning" : "best";
    const suggestedName = suggestMultiCategoryBaganName([sourceCategory, candidate]);

    recommendations.push({
      categoryId: candidate.id,
      categoryName: candidate.nama,
      athleteCount: candAthletes,
      quality,
      score,
      reasons,
      warnings,
      suggestedName,
      parsed: candidateParsed,
    });
  }

  // Sort descending by score (highest compatibility first)
  recommendations.sort((a, b) => b.score - a.score);

  return recommendations;
}

export interface FestivalMixGroupData {
  primary_category_id: number;
  secondary_category_ids: number[];
  bagan_name: string;
}

export interface AutoMixProposal {
  id: string;
  sourceCategory: FestivalCategoryItemLike;
  targetCategories: FestivalCategoryItemLike[];
  proposedBaganName: string;
  quality: MixMatchQuality;
  score: number;
  reasons: string[];
  warnings: string[];
  combinedAthletes: number;
  selected: boolean;
}

export interface AutoMixResult {
  proposals: AutoMixProposal[];
  unmatchedSolos: FestivalCategoryItemLike[];
}

/**
 * Automatically computes intelligent mix proposals for solo categories (1 athlete).
 * Prioritizes:
 * - Pass 1: Solo <-> Solo (1 vs 1)
 * - Pass 2: Solo <-> Small Categories (2 - 3 athletes)
 * - Pass 3: Solo <-> Medium Categories (4 - 12 athletes)
 * - Pass 4: Solo <-> Attach to existing proposal (Tri-category / Mix 3 Kelas)
 */
export function computeAutoMixProposals(
  categories: FestivalCategoryItemLike[],
  existingMixGroups: FestivalMixGroupData[] = []
): AutoMixResult {
  // Build set of already mixed IDs
  const alreadyMixedIds = new Set<number>();
  for (const g of existingMixGroups) {
    alreadyMixedIds.add(g.primary_category_id);
    for (const sid of g.secondary_category_ids) {
      alreadyMixedIds.add(sid);
    }
  }

  // Filter available categories that have athletes
  const availableCategories = categories.filter((c) => {
    const cnt = c.athlete_count ?? c.atlet_count ?? 0;
    return cnt > 0 && !alreadyMixedIds.has(c.id);
  });

  // Find solo categories (1 athlete)
  const solos = availableCategories.filter((c) => {
    const cnt = c.athlete_count ?? c.atlet_count ?? 0;
    return cnt <= 1;
  });

  const proposals: AutoMixProposal[] = [];
  const unmatchedSolos: FestivalCategoryItemLike[] = [];
  const pairedIds = new Set<number>();

  for (const soloA of solos) {
    if (pairedIds.has(soloA.id)) continue;

    // 1. Try to pair with another unmatched Solo first (1 vs 1)
    const candidateSolos = solos.filter(
      (s) => s.id !== soloA.id && !pairedIds.has(s.id)
    );
    const soloRecs = getFestivalMixRecommendations(soloA, candidateSolos);

    let chosenCandidate: FestivalCategoryItemLike | null = null;
    let chosenRec: MixCandidateRecommendation | null = null;

    if (soloRecs.length > 0 && soloRecs[0].quality !== "incompatible") {
      chosenRec = soloRecs[0];
      chosenCandidate = candidateSolos.find((s) => s.id === chosenRec!.categoryId) || null;
    }

    // 2. If no matching solo found, try to pair with small non-solo category (2 - 3 athletes)
    if (!chosenCandidate) {
      const smallCandidates = availableCategories.filter((c) => {
        if (c.id === soloA.id || pairedIds.has(c.id)) return false;
        const cnt = c.athlete_count ?? c.atlet_count ?? 0;
        return cnt >= 2 && cnt <= 3;
      });
      const smallRecs = getFestivalMixRecommendations(soloA, smallCandidates);
      if (smallRecs.length > 0 && smallRecs[0].quality !== "incompatible") {
        chosenRec = smallRecs[0];
        chosenCandidate = smallCandidates.find((c) => c.id === chosenRec!.categoryId) || null;
      }
    }

    // 3. If still no pair found, expand to medium categories (4 - 12 athletes)
    // Avoids stranding athletes when an adjacent tier category has a healthy squad
    if (!chosenCandidate) {
      const mediumCandidates = availableCategories.filter((c) => {
        if (c.id === soloA.id || pairedIds.has(c.id)) return false;
        const cnt = c.athlete_count ?? c.atlet_count ?? 0;
        return cnt >= 4 && cnt <= 12;
      });
      const mediumRecs = getFestivalMixRecommendations(soloA, mediumCandidates);
      if (mediumRecs.length > 0 && mediumRecs[0].quality !== "incompatible") {
        chosenRec = mediumRecs[0];
        chosenCandidate = mediumCandidates.find((c) => c.id === chosenRec!.categoryId) || null;
      }
    }

    if (chosenCandidate && chosenRec) {
      pairedIds.add(soloA.id);
      pairedIds.add(chosenCandidate.id);

      const cntA = soloA.athlete_count ?? soloA.atlet_count ?? 0;
      const cntB = chosenCandidate.athlete_count ?? chosenCandidate.atlet_count ?? 0;

      proposals.push({
        id: `auto-mix-${soloA.id}-${chosenCandidate.id}`,
        sourceCategory: soloA,
        targetCategories: [chosenCandidate],
        proposedBaganName: suggestMultiCategoryBaganName([soloA, chosenCandidate]),
        quality: chosenRec.quality,
        score: chosenRec.score,
        reasons: chosenRec.reasons,
        warnings: chosenRec.warnings,
        combinedAthletes: cntA + cntB,
        selected: true,
      });
    }
  }

  // 4. Pass 4: Multi-Class / Tri-Category Mix
  // Check if any remaining solo can safely be attached to an existing proposal
  for (const soloA of solos) {
    if (pairedIds.has(soloA.id)) continue;

    const sp = parseFestivalCategory(soloA.nama);
    let matchedProposal: AutoMixProposal | null = null;

    for (const p of proposals) {
      // The solo must satisfy the mixing rules (discipline, gender, age tier <= 1, weight <= 10 kg)
      // against EVERY class already in the proposal, not just one of them.
      const allProposalCategories = [p.sourceCategory, ...p.targetCategories];
      const compatible = getFestivalMixRecommendations(soloA, allProposalCategories);
      if (compatible.length !== allProposalCategories.length) continue;

      // Check max combined athletes (limit to 16 athletes)
      const soloCnt = soloA.athlete_count ?? soloA.atlet_count ?? 1;
      if (p.combinedAthletes + soloCnt <= 16 && p.targetCategories.length < 3) {
        matchedProposal = p;
        break;
      }
    }

    if (matchedProposal) {
      pairedIds.add(soloA.id);
      matchedProposal.targetCategories.push(soloA);
      const allCats = [matchedProposal.sourceCategory, ...matchedProposal.targetCategories];
      matchedProposal.proposedBaganName = suggestMultiCategoryBaganName(allCats);
      const soloCnt = soloA.athlete_count ?? soloA.atlet_count ?? 1;
      matchedProposal.combinedAthletes += soloCnt;
      matchedProposal.reasons.push(`Mix 3 Kelas: Ditambahkan ${soloA.nama} (${soloCnt} atlet)`);
      if (matchedProposal.quality !== "warning") {
        const pp = parseFestivalCategory(matchedProposal.sourceCategory.nama);
        if (sp.ageTier !== pp.ageTier) {
          matchedProposal.quality = "warning";
          matchedProposal.warnings.push(`⚠️ Beda usia dengan salah satu kelas gabungan (${soloA.nama})`);
        }
      }
    } else {
      unmatchedSolos.push(soloA);
    }
  }

  return { proposals, unmatchedSolos };
}

/**
 * Adds another category into an existing AutoMixProposal (Mix 3+ Kelas).
 */
export function addCategoryToProposal(
  proposal: AutoMixProposal,
  newCat: FestivalCategoryItemLike
): AutoMixProposal {
  // Prevent duplicate addition
  if (
    proposal.sourceCategory.id === newCat.id ||
    proposal.targetCategories.some((c) => c.id === newCat.id)
  ) {
    return proposal;
  }

  const updatedTargets = [...proposal.targetCategories, newCat];
  const allCats = [proposal.sourceCategory, ...updatedTargets];
  const newBaganName = suggestMultiCategoryBaganName(allCats);
  const newCatCnt = newCat.athlete_count ?? newCat.atlet_count ?? 1;
  const newCombinedAthletes = proposal.combinedAthletes + newCatCnt;

  const sp = parseFestivalCategory(proposal.sourceCategory.nama);
  const np = parseFestivalCategory(newCat.nama);

  const updatedReasons = [...proposal.reasons];
  const updatedWarnings = [...proposal.warnings];

  updatedReasons.push(`Mix ${allCats.length} Kelas: Ditambahkan ${newCat.nama} (${newCatCnt} atlet)`);
  if (sp.ageTier !== np.ageTier && np.ageTier > 0 && sp.ageTier > 0) {
    if (Math.abs(sp.ageTier - np.ageTier) > 1) {
      updatedWarnings.push(`⚠️ Selisih usia > 1 tingkat untuk ${newCat.nama}`);
    } else {
      updatedWarnings.push(`⚠️ Selisih usia 1 tingkat untuk ${newCat.nama}`);
    }
  }

  return {
    ...proposal,
    targetCategories: updatedTargets,
    proposedBaganName: newBaganName,
    combinedAthletes: newCombinedAthletes,
    quality: updatedWarnings.length > 0 ? "warning" : proposal.quality,
    reasons: updatedReasons,
    warnings: updatedWarnings,
  };
}

/**
 * Removes a target category from a proposal.
 */
export function removeTargetCategoryFromProposal(
  proposal: AutoMixProposal,
  targetCatId: number
): AutoMixProposal {
  const updatedTargets = proposal.targetCategories.filter((c) => c.id !== targetCatId);
  const allCats = [proposal.sourceCategory, ...updatedTargets];
  const newBaganName = suggestMultiCategoryBaganName(allCats);
  const removedCat = proposal.targetCategories.find((c) => c.id === targetCatId);
  const removedCnt = removedCat ? (removedCat.athlete_count ?? removedCat.atlet_count ?? 1) : 0;

  return {
    ...proposal,
    targetCategories: updatedTargets,
    proposedBaganName: newBaganName,
    combinedAthletes: Math.max(0, proposal.combinedAthletes - removedCnt),
  };
}
