//! Scientific domain registry (ADR-043 + DOMAIN-REFERENCE-ARXIV.md).
//!
//! Canonical `scientific_domain` codes for multi-domain ontology.
//! Prefer official arXiv category codes when available.
//! Extension namespace `da.*` for non-arXiv domains (medicine, microbiome, etc.).
//!
//! This is COARSE routing + pack selection, not entity identity.
//! GCN, GraphSAGE, PPO are entities; cs.LG is a domain code.

// ─── arXiv Computer Science ───

pub const CS_AI: &str = "cs.AI"; // Artificial Intelligence
pub const CS_LG: &str = "cs.LG"; // Machine Learning (default seed)
pub const CS_CL: &str = "cs.CL"; // Computation and Language (NLP/LLM)
pub const CS_CV: &str = "cs.CV"; // Computer Vision
pub const CS_NE: &str = "cs.NE"; // Neural and Evolutionary Computing
pub const CS_RO: &str = "cs.RO"; // Robotics
pub const CS_IR: &str = "cs.IR"; // Information Retrieval
pub const CS_MA: &str = "cs.MA"; // Multiagent Systems
pub const CS_SI: &str = "cs.SI"; // Social and Information Networks
pub const CS_DS: &str = "cs.DS"; // Data Structures and Algorithms
pub const CS_CR: &str = "cs.CR"; // Cryptography and Security
pub const CS_DB: &str = "cs.DB"; // Databases
pub const CS_DC: &str = "cs.DC"; // Distributed/Parallel/Cluster
pub const CS_SE: &str = "cs.SE"; // Software Engineering
pub const CS_HC: &str = "cs.HC"; // Human-Computer Interaction
pub const CS_CY: &str = "cs.CY"; // Computers and Society
pub const CS_CE: &str = "cs.CE"; // Computational Engineering
pub const CS_GT: &str = "cs.GT"; // Game Theory
pub const CS_IT: &str = "cs.IT"; // Information Theory
pub const CS_LO: &str = "cs.LO"; // Logic in CS
pub const CS_PL: &str = "cs.PL"; // Programming Languages

// ─── arXiv Statistics ───

pub const STAT_ML: &str = "stat.ML"; // Statistics → Machine Learning
pub const STAT_ME: &str = "stat.ME"; // Methodology
pub const STAT_TH: &str = "stat.TH"; // Theory
pub const STAT_AP: &str = "stat.AP"; // Applications
pub const STAT_CO: &str = "stat.CO"; // Computation

// ─── arXiv Mathematics (selected) ───

pub const MATH_OC: &str = "math.OC"; // Optimization and Control
pub const MATH_PR: &str = "math.PR"; // Probability
pub const MATH_ST: &str = "math.ST"; // Statistics Theory
pub const MATH_NA: &str = "math.NA"; // Numerical Analysis
pub const MATH_CO: &str = "math.CO"; // Combinatorics
pub const MATH_LO: &str = "math.LO"; // Logic
pub const MATH_CT: &str = "math.CT"; // Category Theory
pub const MATH_DG: &str = "math.DG"; // Differential Geometry
pub const MATH_DS: &str = "math.DS"; // Dynamical Systems

// ─── arXiv Physics (selected) ───

pub const PHYS_COMP_PH: &str = "physics.comp-ph"; // Computational Physics
pub const PHYS_DATA_AN: &str = "physics.data-an"; // Data Analysis
pub const PHYS_BIO_PH: &str = "physics.bio-ph"; // Biological Physics
pub const PHYS_CHEM_PH: &str = "physics.chem-ph"; // Chemical Physics
pub const PHYS_SOC_PH: &str = "physics.soc-ph"; // Physics and Society
pub const PHYS_MED_PH: &str = "physics.med-ph"; // Medical Physics
pub const QUANT_PH: &str = "quant-ph"; // Quantum Physics
pub const CONDMAT_DIS_NN: &str = "cond-mat.dis-nn"; // Disordered Systems/NN
pub const CONDMAT_STAT_MECH: &str = "cond-mat.stat-mech"; // Statistical Mechanics
pub const HEP_TH: &str = "hep-th"; // High Energy Physics - Theory
pub const GR_QC: &str = "gr-qc"; // General Relativity

// ─── arXiv Quantitative Biology ───

pub const QBIO_BM: &str = "q-bio.BM"; // Biomolecules
pub const QBIO_CB: &str = "q-bio.CB"; // Cell Behavior
pub const QBIO_GN: &str = "q-bio.GN"; // Genomics
pub const QBIO_MN: &str = "q-bio.MN"; // Molecular Networks
pub const QBIO_NC: &str = "q-bio.NC"; // Neurons and Cognition
pub const QBIO_PE: &str = "q-bio.PE"; // Populations and Evolution
pub const QBIO_QM: &str = "q-bio.QM"; // Quantitative Methods
pub const QBIO_SC: &str = "q-bio.SC"; // Subcellular Processes
pub const QBIO_TO: &str = "q-bio.TO"; // Tissues and Organs

// ─── arXiv q-fin / eess / econ (selected) ───

pub const QFIN_CP: &str = "q-fin.CP"; // Computational Finance
pub const QFIN_PM: &str = "q-fin.PM"; // Portfolio Management
pub const QFIN_ST: &str = "q-fin.ST"; // Statistical Finance
pub const EESS_AS: &str = "eess.AS"; // Audio and Speech
pub const EESS_IV: &str = "eess.IV"; // Image and Video Processing
pub const EESS_SP: &str = "eess.SP"; // Signal Processing
pub const EESS_SY: &str = "eess.SY"; // Systems and Control
pub const ECON_EM: &str = "econ.EM"; // Econometrics
pub const ECON_GN: &str = "econ.GN"; // General Economics
pub const ECON_TH: &str = "econ.TH"; // Theoretical Economics

// ─── Extension namespace: non-arXiv domains (da.*) ───
//
// First-class domains not well-covered by arXiv.
// These are NOT second-class — they have equal status with arXiv codes.

pub const DA_MEDICINE: &str = "da.medicine"; // Clinical/biomedical
pub const DA_MICROBIOME: &str = "da.microbiome"; // Host-microbiome
pub const DA_METABOLISM: &str = "da.metabolism"; // Metabolic health
pub const DA_GENETICS: &str = "da.genetics"; // Applied genetics
pub const DA_BIOHACKING: &str = "da.biohacking"; // Enhancement/self-experiment
pub const DA_NUTRITION: &str = "da.nutrition"; // Nutrition science
pub const DA_LONGEVITY: &str = "da.longevity"; // Aging/longevity
pub const DA_SOCIAL_SCIENCE: &str = "da.social_science"; // Social/behavioral
pub const DA_CHEMISTRY: &str = "da.chemistry"; // Applied chemistry
pub const DA_GENERAL: &str = "da.general"; // Fallback for unknown

/// Check if a domain code is a known arXiv code.
pub fn is_known_arxiv(code: &str) -> bool {
    matches!(
        code,
        CS_AI
            | CS_LG
            | CS_CL
            | CS_CV
            | CS_NE
            | CS_RO
            | CS_IR
            | CS_MA
            | CS_SI
            | CS_DS
            | CS_CR
            | CS_DB
            | CS_DC
            | CS_SE
            | CS_HC
            | CS_CY
            | CS_CE
            | CS_GT
            | CS_IT
            | CS_LO
            | CS_PL
            | STAT_ML
            | STAT_ME
            | STAT_TH
            | STAT_AP
            | STAT_CO
            | MATH_OC
            | MATH_PR
            | MATH_ST
            | MATH_NA
            | MATH_CO
            | MATH_LO
            | MATH_CT
            | MATH_DG
            | MATH_DS
            | PHYS_COMP_PH
            | PHYS_DATA_AN
            | PHYS_BIO_PH
            | PHYS_CHEM_PH
            | PHYS_SOC_PH
            | PHYS_MED_PH
            | QUANT_PH
            | CONDMAT_DIS_NN
            | CONDMAT_STAT_MECH
            | HEP_TH
            | GR_QC
            | QBIO_BM
            | QBIO_CB
            | QBIO_GN
            | QBIO_MN
            | QBIO_NC
            | QBIO_PE
            | QBIO_QM
            | QBIO_SC
            | QBIO_TO
            | QFIN_CP
            | QFIN_PM
            | QFIN_ST
            | EESS_AS
            | EESS_IV
            | EESS_SP
            | EESS_SY
            | ECON_EM
            | ECON_GN
            | ECON_TH
    )
}

/// Check if a domain code is a known extension (da.*) code.
pub fn is_known_extension(code: &str) -> bool {
    matches!(
        code,
        DA_MEDICINE
            | DA_MICROBIOME
            | DA_METABOLISM
            | DA_GENETICS
            | DA_BIOHACKING
            | DA_NUTRITION
            | DA_LONGEVITY
            | DA_SOCIAL_SCIENCE
            | DA_CHEMISTRY
            | DA_GENERAL
    )
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
        "cs.ml" | "machine-learning" | "machine_learning" | "ml" => CS_LG,
        "nlp" | "cs.nlp" => CS_CL,
        "cv" | "computer-vision" | "computer_vision" => CS_CV,
        "gnn" | "graph-ml" | "graph_ml" => CS_LG, // GNN is a topic, routes to cs.LG
        "rl" | "reinforcement-learning" | "reinforcement_learning" => CS_LG,
        "biohacking" => DA_BIOHACKING,
        "microbiome" => DA_MICROBIOME,
        "metabolism" | "metabolic" => DA_METABOLISM,
        "medicine" | "medical" | "clinical" => DA_MEDICINE,
        "nutrition" | "nutritional" => DA_NUTRITION,
        "genetics" | "genomic" | "genomics" => DA_GENETICS,
        "longevity" | "aging" => DA_LONGEVITY,
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
        // GNN is a topic, not a domain — routes to cs.LG
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
        // Must be cs.LG not cs.ml or cs.ML
        assert_eq!(CS_LG, "cs.LG");
    }

    #[test]
    fn test_qbio_codes_exist() {
        assert!(is_known(QBIO_GN));
        assert!(is_known(QBIO_QM));
    }

    #[test]
    fn test_da_codes_not_in_arxiv() {
        for code in [DA_MEDICINE, DA_MICROBIOME, DA_METABOLISM, DA_BIOHACKING] {
            assert!(!is_known_arxiv(code));
            assert!(is_known_extension(code));
        }
    }
}
