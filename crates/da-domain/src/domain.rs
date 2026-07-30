//! Scientific domain registry (ADR-043 + DOMAIN-REFERENCE-ARXIV.md).
//!
//! Complete official arXiv category taxonomy (154 categories) as canonical
//! `scientific_domain` codes for multi-domain ontology.
//! Extension namespace `da.*` for non-arXiv domains (medicine, microbiome, etc.).
//!
//! Source: https://arxiv.org/category_taxonomy (verified 2026-07-29)
//!
//! This is COARSE routing + pack selection, not entity identity.
//! GCN, GraphSAGE, PPO are entities; cs.LG is a domain code.

// ═══════════════════════════════════════════════════════════════
// OFFICIAL ARXIV CATEGORY TAXONOMY (154 categories)
// ═══════════════════════════════════════════════════════════════

// ─── arXiv astro-ph (6) — Astrophysics ───
pub const ASTRO_PH_CO: &str = "astro-ph.CO"; // Cosmology and Nongalactic Astrophysics
pub const ASTRO_PH_EP: &str = "astro-ph.EP"; // Earth and Planetary Astrophysics
pub const ASTRO_PH_GA: &str = "astro-ph.GA"; // Astrophysics of Galaxies
pub const ASTRO_PH_HE: &str = "astro-ph.HE"; // High Energy Astrophysical Phenomena
pub const ASTRO_PH_IM: &str = "astro-ph.IM"; // Instrumentation and Methods for Astrophysics
pub const ASTRO_PH_SR: &str = "astro-ph.SR"; // Solar and Stellar Astrophysics

// ─── arXiv q-bio (10) — Quantitative Biology ───
pub const QBIO_BM: &str = "q-bio.BM"; // Biomolecules
pub const QBIO_CB: &str = "q-bio.CB"; // Cell Behavior
pub const QBIO_GN: &str = "q-bio.GN"; // Genomics
pub const QBIO_MN: &str = "q-bio.MN"; // Molecular Networks
pub const QBIO_NC: &str = "q-bio.NC"; // Neurons and Cognition
pub const QBIO_OT: &str = "q-bio.OT"; // Other Quantitative Biology
pub const QBIO_PE: &str = "q-bio.PE"; // Populations and Evolution
pub const QBIO_QM: &str = "q-bio.QM"; // Quantitative Methods
pub const QBIO_SC: &str = "q-bio.SC"; // Subcellular Processes
pub const QBIO_TO: &str = "q-bio.TO"; // Tissues and Organs

// ─── arXiv cond-mat (9) — Condensed Matter ───
pub const CONDMAT_DIS_NN: &str = "cond-mat.dis-nn"; // Disordered Systems and Neural Networks
pub const CONDMAT_MES_HALL: &str = "cond-mat.mes-hall"; // Mesoscale and Nanoscale Physics
pub const CONDMAT_MTRL_SCI: &str = "cond-mat.mtrl-sci"; // Materials Science
pub const CONDMAT_OTHER: &str = "cond-mat.other"; // Other Condensed Matter
pub const CONDMAT_QUANT_GAS: &str = "cond-mat.quant-gas"; // Quantum Gases
pub const CONDMAT_SOFT: &str = "cond-mat.soft"; // Soft Condensed Matter
pub const CONDMAT_STAT_MECH: &str = "cond-mat.stat-mech"; // Statistical Mechanics
pub const CONDMAT_STR_EL: &str = "cond-mat.str-el"; // Strongly Correlated Electrons
pub const CONDMAT_SUPR_CON: &str = "cond-mat.supr-con"; // Superconductivity

// ─── arXiv cs (40) — Computer Science ───
pub const CS_AI: &str = "cs.AI"; // Artificial Intelligence
pub const CS_AR: &str = "cs.AR"; // Hardware Architecture
pub const CS_CC: &str = "cs.CC"; // Computational Complexity
pub const CS_CE: &str = "cs.CE"; // Computational Engineering, Finance, and Science
pub const CS_CG: &str = "cs.CG"; // Computational Geometry
pub const CS_CL: &str = "cs.CL"; // Computation and Language
pub const CS_CR: &str = "cs.CR"; // Cryptography and Security
pub const CS_CV: &str = "cs.CV"; // Computer Vision and Pattern Recognition
pub const CS_CY: &str = "cs.CY"; // Computers and Society
pub const CS_DB: &str = "cs.DB"; // Databases
pub const CS_DC: &str = "cs.DC"; // Distributed, Parallel, and Cluster Computing
pub const CS_DL: &str = "cs.DL"; // Digital Libraries
pub const CS_DM: &str = "cs.DM"; // Discrete Mathematics
pub const CS_DS: &str = "cs.DS"; // Data Structures and Algorithms
pub const CS_ET: &str = "cs.ET"; // Emerging Technologies
pub const CS_FL: &str = "cs.FL"; // Formal Languages and Automata Theory
pub const CS_GL: &str = "cs.GL"; // General Literature
pub const CS_GR: &str = "cs.GR"; // Graphics
pub const CS_GT: &str = "cs.GT"; // Computer Science and Game Theory
pub const CS_HC: &str = "cs.HC"; // Human-Computer Interaction
pub const CS_IR: &str = "cs.IR"; // Information Retrieval
pub const CS_IT: &str = "cs.IT"; // Information Theory
pub const CS_LG: &str = "cs.LG"; // Machine Learning (seed corpus)
pub const CS_LO: &str = "cs.LO"; // Logic in Computer Science
pub const CS_MA: &str = "cs.MA"; // Multiagent Systems
pub const CS_MM: &str = "cs.MM"; // Multimedia
pub const CS_MS: &str = "cs.MS"; // Mathematical Software
pub const CS_NA: &str = "cs.NA"; // Numerical Analysis
pub const CS_NE: &str = "cs.NE"; // Neural and Evolutionary Computing
pub const CS_NI: &str = "cs.NI"; // Networking and Internet Architecture
pub const CS_OH: &str = "cs.OH"; // Other Computer Science
pub const CS_OS: &str = "cs.OS"; // Operating Systems
pub const CS_PF: &str = "cs.PF"; // Performance
pub const CS_PL: &str = "cs.PL"; // Programming Languages
pub const CS_RO: &str = "cs.RO"; // Robotics
pub const CS_SC: &str = "cs.SC"; // Symbolic Computation
pub const CS_SD: &str = "cs.SD"; // Sound
pub const CS_SE: &str = "cs.SE"; // Software Engineering
pub const CS_SI: &str = "cs.SI"; // Social and Information Networks
pub const CS_SY: &str = "cs.SY"; // Systems and Control

// ─── arXiv econ (3) — Economics ───
pub const ECON_EM: &str = "econ.EM"; // Econometrics
pub const ECON_GN: &str = "econ.GN"; // General Economics
pub const ECON_TH: &str = "econ.TH"; // Theoretical Economics

// ─── arXiv eess (4) — Electrical Engineering and Systems Science ───
pub const EESS_AS: &str = "eess.AS"; // Audio and Speech Processing
pub const EESS_IV: &str = "eess.IV"; // Image and Video Processing
pub const EESS_SP: &str = "eess.SP"; // Signal Processing
pub const EESS_SY: &str = "eess.SY"; // Systems and Control

// ─── arXiv fin (9) — Quantitative Finance (replaced q-fin) ───
pub const FIN_CP: &str = "fin.CP"; // Computational Finance
pub const FIN_EC: &str = "fin.EC"; // Economics
pub const FIN_GN: &str = "fin.GN"; // General Finance
pub const FIN_MF: &str = "fin.MF"; // Mathematical Finance
pub const FIN_PM: &str = "fin.PM"; // Portfolio Management
pub const FIN_PR: &str = "fin.PR"; // Pricing of Securities
pub const FIN_RM: &str = "fin.RM"; // Risk Management
pub const FIN_ST: &str = "fin.ST"; // Statistical Finance
pub const FIN_TR: &str = "fin.TR"; // Trading and Market Microstructure

// ─── arXiv math (32) — Mathematics ───
pub const MATH_AC: &str = "math.AC"; // Algebraic Geometry
pub const MATH_AG: &str = "math.AG"; // Algebraic Geometry
pub const MATH_AP: &str = "math.AP"; // Analysis of PDEs
pub const MATH_AT: &str = "math.AT"; // Algebraic Topology
pub const MATH_CA: &str = "math.CA"; // Classical Analysis and ODEs
pub const MATH_CO: &str = "math.CO"; // Combinatorics
pub const MATH_CT: &str = "math.CT"; // Category Theory
pub const MATH_CV: &str = "math.CV"; // Complex Variables
pub const MATH_DG: &str = "math.DG"; // Differential Geometry
pub const MATH_DS: &str = "math.DS"; // Dynamical Systems
pub const MATH_FA: &str = "math.FA"; // Functional Analysis
pub const MATH_GM: &str = "math.GM"; // General Mathematics
pub const MATH_GN: &str = "math.GN"; // General Topology
pub const MATH_GR: &str = "math.GR"; // Group Theory
pub const MATH_GT: &str = "math.GT"; // Geometric Topology
pub const MATH_HO: &str = "math.HO"; // History and Overview
pub const MATH_IT: &str = "math.IT"; // Information Theory
pub const MATH_KT: &str = "math.KT"; // K-Theory and Homology
pub const MATH_LO: &str = "math.LO"; // Logic
pub const MATH_MG: &str = "math.MG"; // Metric Geometry
pub const MATH_MP: &str = "math.MP"; // Mathematical Physics
pub const MATH_NA: &str = "math.NA"; // Numerical Analysis
pub const MATH_NT: &str = "math.NT"; // Number Theory
pub const MATH_OA: &str = "math.OA"; // Operator Algebras
pub const MATH_OC: &str = "math.OC"; // Optimization and Control
pub const MATH_PR: &str = "math.PR"; // Probability
pub const MATH_QA: &str = "math.QA"; // Quantum Algebra
pub const MATH_RA: &str = "math.RA"; // Rings and Algebras
pub const MATH_RT: &str = "math.RT"; // Representation Theory
pub const MATH_SG: &str = "math.SG"; // Symplectic Geometry
pub const MATH_SP: &str = "math.SP"; // Spectral Theory
pub const MATH_ST: &str = "math.ST"; // Statistics Theory

// ─── arXiv nlin (5) — Nonlinear Sciences ───
pub const NLIN_AO: &str = "nlin.AO"; // Adaptation and Self-Organizing Systems
pub const NLIN_CD: &str = "nlin.CD"; // Chaotic Dynamics
pub const NLIN_CG: &str = "nlin.CG"; // Cellular Automata and Lattice Gases
pub const NLIN_PS: &str = "nlin.PS"; // Pattern Formation and Solitons
pub const NLIN_SI: &str = "nlin.SI"; // Exactly Solvable and Integrable Systems

// ─── arXiv physics (22) — Physics (general) ───
pub const PHYS_ACC_PH: &str = "physics.acc-ph"; // Accelerator Physics
pub const PHYS_AO_PH: &str = "physics.ao-ph"; // Atmospheric and Oceanic Physics
pub const PHYS_APP_PH: &str = "physics.app-ph"; // Applied Physics
pub const PHYS_ATM_CLUS: &str = "physics.atm-clus"; // Atomic and Molecular Clusters
pub const PHYS_ATOM_PH: &str = "physics.atom-ph"; // Atomic Physics
pub const PHYS_BIO_PH: &str = "physics.bio-ph"; // Biological Physics
pub const PHYS_CHEM_PH: &str = "physics.chem-ph"; // Chemical Physics
pub const PHYS_CLASS_PH: &str = "physics.class-ph"; // Classical Physics
pub const PHYS_COMP_PH: &str = "physics.comp-ph"; // Computational Physics
pub const PHYS_DATA_AN: &str = "physics.data-an"; // Data Analysis, Statistics and Probability
pub const PHYS_ED_PH: &str = "physics.ed-ph"; // Physics Education
pub const PHYS_FLU_DYN: &str = "physics.flu-dyn"; // Fluid Dynamics
pub const PHYS_GEN_PH: &str = "physics.gen-ph"; // General Physics
pub const PHYS_GEO_PH: &str = "physics.geo-ph"; // Geophysics
pub const PHYS_HIST_PH: &str = "physics.hist-ph"; // History and Philosophy of Physics
pub const PHYS_INS_DET: &str = "physics.ins-det"; // Instrumentation and Detectors
pub const PHYS_MED_PH: &str = "physics.med-ph"; // Medical Physics
pub const PHYS_OPTICS: &str = "physics.optics"; // Optics
pub const PHYS_PLASM_PH: &str = "physics.plasm-ph"; // Plasma Physics
pub const PHYS_POP_PH: &str = "physics.pop-ph"; // Popular Physics
pub const PHYS_SOC_PH: &str = "physics.soc-ph"; // Physics and Society
pub const PHYS_SPACE_PH: &str = "physics.space-ph"; // Space Physics

// ─── arXiv stat (6) — Statistics ───
pub const STAT_AP: &str = "stat.AP"; // Applications
pub const STAT_CO: &str = "stat.CO"; // Computation
pub const STAT_ME: &str = "stat.ME"; // Methodology
pub const STAT_ML: &str = "stat.ML"; // Machine Learning
pub const STAT_OT: &str = "stat.OT"; // Other Statistics
pub const STAT_TH: &str = "stat.TH"; // Statistics Theory

// ─── arXiv standalone categories (7) ───
pub const GR_QC: &str = "gr-qc"; // General Relativity and Quantum Cosmology
pub const HEP_EX: &str = "hep-ex"; // High Energy Physics - Experiment
pub const HEP_LAT: &str = "hep-lat"; // High Energy Physics - Lattice
pub const HEP_PH: &str = "hep-ph"; // High Energy Physics - Phenomenology
pub const HEP_TH: &str = "hep-th"; // High Energy Physics - Theory
pub const NUCL_EX: &str = "nucl-ex"; // Nuclear Experiment
pub const NUCL_TH: &str = "nucl-th"; // Nuclear Theory
pub const QUANT_PH: &str = "quant-ph"; // Quantum Physics

// ═══════════════════════════════════════════════════════════════
// EXTENSION NAMESPACE: da.* (non-arXiv first-class domains)
// ═══════════════════════════════════════════════════════════════
//
// Domains not well-covered by arXiv but critical for this project.
// Equal status with arXiv codes — NOT second-class.

pub const DA_MEDICINE: &str = "da.medicine"; // Clinical / biomedical research
pub const DA_MICROBIOME: &str = "da.microbiome"; // Host-microbiome science
pub const DA_METABOLISM: &str = "da.metabolism"; // Metabolism / metabolic health
pub const DA_GENETICS: &str = "da.genetics"; // Applied genetics / genomics
pub const DA_BIOHACKING: &str = "da.biohacking"; // Enhancement / self-experiment
pub const DA_NUTRITION: &str = "da.nutrition"; // Nutrition science
pub const DA_LONGEVITY: &str = "da.longevity"; // Aging / longevity research
pub const DA_SOCIAL_SCIENCE: &str = "da.social_science"; // Social / behavioral sciences
pub const DA_CHEMISTRY: &str = "da.chemistry"; // Applied chemistry
pub const DA_GENERAL: &str = "da.general"; // Fallback for unknown

// ═══════════════════════════════════════════════════════════════
// LOOKUP FUNCTIONS
// ═══════════════════════════════════════════════════════════════

/// All official arXiv category codes (154 categories).
pub const ALL_ARXIV_CODES: &[&str] = &[
    // astro-ph
    ASTRO_PH_CO,
    ASTRO_PH_EP,
    ASTRO_PH_GA,
    ASTRO_PH_HE,
    ASTRO_PH_IM,
    ASTRO_PH_SR,
    // q-bio
    QBIO_BM,
    QBIO_CB,
    QBIO_GN,
    QBIO_MN,
    QBIO_NC,
    QBIO_OT,
    QBIO_PE,
    QBIO_QM,
    QBIO_SC,
    QBIO_TO,
    // cond-mat
    CONDMAT_DIS_NN,
    CONDMAT_MES_HALL,
    CONDMAT_MTRL_SCI,
    CONDMAT_OTHER,
    CONDMAT_QUANT_GAS,
    CONDMAT_SOFT,
    CONDMAT_STAT_MECH,
    CONDMAT_STR_EL,
    CONDMAT_SUPR_CON,
    // cs
    CS_AI,
    CS_AR,
    CS_CC,
    CS_CE,
    CS_CG,
    CS_CL,
    CS_CR,
    CS_CV,
    CS_CY,
    CS_DB,
    CS_DC,
    CS_DL,
    CS_DM,
    CS_DS,
    CS_ET,
    CS_FL,
    CS_GL,
    CS_GR,
    CS_GT,
    CS_HC,
    CS_IR,
    CS_IT,
    CS_LG,
    CS_LO,
    CS_MA,
    CS_MM,
    CS_MS,
    CS_NA,
    CS_NE,
    CS_NI,
    CS_OH,
    CS_OS,
    CS_PF,
    CS_PL,
    CS_RO,
    CS_SC,
    CS_SD,
    CS_SE,
    CS_SI,
    CS_SY,
    // econ
    ECON_EM,
    ECON_GN,
    ECON_TH,
    // eess
    EESS_AS,
    EESS_IV,
    EESS_SP,
    EESS_SY,
    // fin
    FIN_CP,
    FIN_EC,
    FIN_GN,
    FIN_MF,
    FIN_PM,
    FIN_PR,
    FIN_RM,
    FIN_ST,
    FIN_TR,
    // math
    MATH_AC,
    MATH_AG,
    MATH_AP,
    MATH_AT,
    MATH_CA,
    MATH_CO,
    MATH_CT,
    MATH_CV,
    MATH_DG,
    MATH_DS,
    MATH_FA,
    MATH_GM,
    MATH_GN,
    MATH_GR,
    MATH_GT,
    MATH_HO,
    MATH_IT,
    MATH_KT,
    MATH_LO,
    MATH_MG,
    MATH_MP,
    MATH_NA,
    MATH_NT,
    MATH_OA,
    MATH_OC,
    MATH_PR,
    MATH_QA,
    MATH_RA,
    MATH_RT,
    MATH_SG,
    MATH_SP,
    MATH_ST,
    // nlin
    NLIN_AO,
    NLIN_CD,
    NLIN_CG,
    NLIN_PS,
    NLIN_SI,
    // physics
    PHYS_ACC_PH,
    PHYS_AO_PH,
    PHYS_APP_PH,
    PHYS_ATM_CLUS,
    PHYS_ATOM_PH,
    PHYS_BIO_PH,
    PHYS_CHEM_PH,
    PHYS_CLASS_PH,
    PHYS_COMP_PH,
    PHYS_DATA_AN,
    PHYS_ED_PH,
    PHYS_FLU_DYN,
    PHYS_GEN_PH,
    PHYS_GEO_PH,
    PHYS_HIST_PH,
    PHYS_INS_DET,
    PHYS_MED_PH,
    PHYS_OPTICS,
    PHYS_PLASM_PH,
    PHYS_POP_PH,
    PHYS_SOC_PH,
    PHYS_SPACE_PH,
    // stat
    STAT_AP,
    STAT_CO,
    STAT_ME,
    STAT_ML,
    STAT_OT,
    STAT_TH,
    // standalone
    GR_QC,
    HEP_EX,
    HEP_LAT,
    HEP_PH,
    HEP_TH,
    NUCL_EX,
    NUCL_TH,
    QUANT_PH,
];

/// All extension domain codes (da.*).
pub const ALL_DA_CODES: &[&str] = &[
    DA_MEDICINE,
    DA_MICROBIOME,
    DA_METABOLISM,
    DA_GENETICS,
    DA_BIOHACKING,
    DA_NUTRITION,
    DA_LONGEVITY,
    DA_SOCIAL_SCIENCE,
    DA_CHEMISTRY,
    DA_GENERAL,
];

/// Check if a domain code is a known arXiv code.
pub fn is_known_arxiv(code: &str) -> bool {
    ALL_ARXIV_CODES.contains(&code)
}

/// Check if a domain code is a known extension (da.*) code.
pub fn is_known_extension(code: &str) -> bool {
    ALL_DA_CODES.contains(&code)
}

/// Check if a domain code is recognized (arXiv or extension).
pub fn is_known(code: &str) -> bool {
    is_known_arxiv(code) || is_known_extension(code)
}

/// Canonicalize an informal domain label.
/// Returns the canonical code if recognized, or the input as-is if unknown.
pub fn canonicalize(input: &str) -> &str {
    let lower = input.to_lowercase();
    match lower.as_str() {
        // CS aliases
        "cs.ml" | "machine-learning" | "machine_learning" | "ml" => CS_LG,
        "nlp" | "cs.nlp" => CS_CL,
        "cv" | "computer-vision" | "computer_vision" => CS_CV,
        "gnn" | "graph-ml" | "graph_ml" => CS_LG, // GNN is a topic, routes to cs.LG
        "rl" | "reinforcement-learning" | "reinforcement_learning" => CS_LG,
        // Extension aliases
        "biohacking" => DA_BIOHACKING,
        "microbiome" => DA_MICROBIOME,
        "metabolism" | "metabolic" => DA_METABOLISM,
        "medicine" | "medical" | "clinical" => DA_MEDICINE,
        "nutrition" | "nutritional" => DA_NUTRITION,
        "genetics" | "genomic" | "genomics" => DA_GENETICS,
        "longevity" | "aging" => DA_LONGEVITY,
        // Legacy q-fin → fin migration
        "q-fin.cp" => FIN_CP,
        "q-fin.pm" => FIN_PM,
        "q-fin.st" => FIN_ST,
        _ => input,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cs_lg_is_known() {
        assert!(is_known(CS_LG));
        assert!(is_known_arxiv(CS_LG));
    }

    #[test]
    fn test_da_medicine_is_known() {
        assert!(is_known(DA_MEDICINE));
        assert!(is_known_extension(DA_MEDICINE));
        assert!(!is_known_arxiv(DA_MEDICINE));
    }

    #[test]
    fn test_unknown_not_known() {
        assert!(!is_known("xx.YY"));
    }

    #[test]
    fn test_canonicalize_cs_ml_to_cs_lg() {
        assert_eq!(canonicalize("cs.ml"), CS_LG);
        assert_eq!(canonicalize("machine-learning"), CS_LG);
    }

    #[test]
    fn test_canonicalize_nlp() {
        assert_eq!(canonicalize("nlp"), CS_CL);
    }

    #[test]
    fn test_canonicalize_gnn() {
        assert_eq!(canonicalize("gnn"), CS_LG);
    }

    #[test]
    fn test_canonicalize_medicine() {
        assert_eq!(canonicalize("medicine"), DA_MEDICINE);
        assert_eq!(canonicalize("clinical"), DA_MEDICINE);
    }

    #[test]
    fn test_canonicalize_unknown_passthrough() {
        assert_eq!(canonicalize("xx.YY"), "xx.YY");
    }

    #[test]
    fn test_cs_lg_exact_spelling() {
        assert_eq!(CS_LG, "cs.LG");
    }

    #[test]
    fn test_qbio_codes_exist() {
        assert!(is_known(QBIO_GN));
        assert!(is_known(QBIO_QM));
        assert!(is_known(QBIO_OT));
    }

    #[test]
    fn test_fin_replaced_qfin() {
        // q-fin.* was replaced by fin.* in arXiv taxonomy
        assert!(is_known(FIN_CP));
        assert!(is_known(FIN_ST));
        assert!(!is_known("q-fin.CP")); // old code should NOT be valid
    }

    #[test]
    fn test_da_codes_not_in_arxiv() {
        for code in [DA_MEDICINE, DA_MICROBIOME, DA_METABOLISM, DA_BIOHACKING] {
            assert!(!is_known_arxiv(code));
            assert!(is_known_extension(code));
        }
    }

    #[test]
    fn test_all_154_arxiv_codes_known() {
        // All 154 official arXiv categories must be recognized
        assert_eq!(ALL_ARXIV_CODES.len(), 154);
        for code in ALL_ARXIV_CODES {
            assert!(is_known_arxiv(code), "{code} not recognized");
        }
    }

    #[test]
    fn test_all_10_da_codes_known() {
        assert_eq!(ALL_DA_CODES.len(), 10);
        for code in ALL_DA_CODES {
            assert!(is_known_extension(code), "{code} not recognized");
        }
    }

    #[test]
    fn test_cs_has_40_codes() {
        let cs_count = ALL_ARXIV_CODES
            .iter()
            .filter(|c| c.starts_with("cs."))
            .count();
        assert_eq!(cs_count, 40);
    }

    #[test]
    fn test_math_has_32_codes() {
        let math_count = ALL_ARXIV_CODES
            .iter()
            .filter(|c| c.starts_with("math."))
            .count();
        assert_eq!(math_count, 32);
    }

    #[test]
    fn test_physics_has_22_codes() {
        let phys_count = ALL_ARXIV_CODES
            .iter()
            .filter(|c| c.starts_with("physics."))
            .count();
        assert_eq!(phys_count, 22);
    }

    #[test]
    fn test_qfin_legacy_migration() {
        assert_eq!(canonicalize("q-fin.CP"), FIN_CP);
    }
}
