use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::core::pipeline::{DeobfuscationConfig, DeobfuscatorEngine};

/// Status of an individual analysis phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseStatus {
    Pending,
    Running,
    Completed,
    Skipped,
    Warning,
    Error,
}

impl PhaseStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Running => "RUNNING",
            Self::Completed => "COMPLETED",
            Self::Skipped => "SKIPPED",
            Self::Warning => "WARNING",
            Self::Error => "ERROR",
        }
    }
}

/// Information tracking an individual phase.
#[derive(Debug, Clone)]
pub struct PhaseState {
    pub index: usize,
    pub name: String,
    pub description: String,
    pub status: PhaseStatus,
    pub detail: String,
    pub elapsed_ms: u64,
}

/// Real-time progress update event delivered to the UI.
#[derive(Debug, Clone)]
pub struct AnalysisProgressUpdate {
    pub current_phase: usize,
    pub progress_percent: f32, // 0.0 .. 100.0
    pub current_operation: String,
    pub files_processed: usize,
    pub files_total: usize,
    pub phases: Vec<PhaseState>,
    pub live_log: String,
}

/// Inspection record of an archive (ZIP/JAR).
#[derive(Debug, Clone)]
pub struct ArchiveInspection {
    pub file_path: String,
    pub file_count: usize,
    pub uncompressed_bytes: u64,
    pub nested_archives: Vec<String>,
    pub main_class: Option<String>,
    pub manifest_entries: Vec<String>,
}

/// Concrete technical evidence of obfuscation.
#[derive(Debug, Clone)]
pub struct ObfuscationFinding {
    pub file_path: String,
    pub line_number: usize,
    pub pattern_name: String,
    pub evidence: String,
    pub severity: String,
}

/// Security threat classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityClassification {
    ConfirmedThreat,
    SuspiciousIndicator,
    PotentiallyDangerousCall,
    Clean,
    Incomplete,
}

impl SecurityClassification {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ConfirmedThreat => "Amenaza confirmada",
            Self::SuspiciousIndicator => "Indicador sospechoso",
            Self::PotentiallyDangerousCall => "Llamada potencialmente peligrosa",
            Self::Clean => "Sin hallazgos detectados",
            Self::Incomplete => "Análisis incompleto",
        }
    }
}

/// Security finding with technical justification.
#[derive(Debug, Clone)]
pub struct SecurityFinding {
    pub file_path: String,
    pub line_number: usize,
    pub classification: SecurityClassification,
    pub sink_pattern: String,
    pub description: String,
    pub confidence: String,
    pub recommendation: String,
}

/// Categorization of the final project analysis result.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisVerdictCase {
    CaseA_Obfuscated,
    CaseB_CleanProject,
    CaseC_IncompatibleFiles,
    CaseD_SecurityThreat,
}

/// Final consolidated report produced by the 7-phase analysis engine.
#[derive(Debug, Clone)]
pub struct ProjectAnalysisReport {
    pub project_name: String,
    pub project_path: String,
    pub total_files: usize,
    pub total_dirs: usize,
    pub total_bytes: u64,
    pub total_elapsed_ms: u64,
    pub language_distribution: Vec<(String, usize)>,
    pub archives: Vec<ArchiveInspection>,
    pub obfuscation_findings: Vec<ObfuscationFinding>,
    pub security_findings: Vec<SecurityFinding>,
    pub incompatible_files: Vec<(String, String)>,
    pub reconstructed_count: usize,
    pub verdict_case: AnalysisVerdictCase,
    pub verdict_headline: String,
    pub verdict_summary: String,
    pub markdown_report: String,
}

impl ProjectAnalysisReport {
    /// Formats real 7-phase analysis findings into structured context for KRONOS AI prompts.
    pub fn format_for_ai_prompt(&self) -> String {
        let verdict_label = match self.verdict_case {
            AnalysisVerdictCase::CaseA_Obfuscated => "Caso A: Código Ofuscado",
            AnalysisVerdictCase::CaseB_CleanProject => "Caso B: Proyecto Limpio",
            AnalysisVerdictCase::CaseC_IncompatibleFiles => "Caso C: Archivos Incompatibles",
            AnalysisVerdictCase::CaseD_SecurityThreat => "Caso D: Amenaza de Seguridad",
        };

        let mut ctx = format!(
            "\n--- CONTEXTO DETERMINISTA DEL PROYECTO (KRONOS DESCODE ENGINE) ---\n\
            Proyecto: {}\n\
            Ruta: {}\n\
            Total archivos: {} ({} directorios, {} bytes)\n\
            Veredicto motor: {} - {}\n\
            Resumen: {}\n",
            self.project_name,
            self.project_path,
            self.total_files,
            self.total_dirs,
            self.total_bytes,
            verdict_label,
            self.verdict_headline,
            self.verdict_summary
        );

        if !self.obfuscation_findings.is_empty() {
            ctx.push_str("\nHallazgos técnicos de ofuscación:\n");
            for f in self.obfuscation_findings.iter().take(10) {
                ctx.push_str(&format!(
                    "- [{}] {} (Línea {}): {}\n",
                    f.severity, f.file_path, f.line_number, f.pattern_name
                ));
            }
        }

        if !self.security_findings.is_empty() {
            ctx.push_str("\nHallazgos técnicos de seguridad:\n");
            for s in self.security_findings.iter().take(10) {
                ctx.push_str(&format!(
                    "- [{}] {} (Línea {}): {} -> {}\n",
                    s.classification.as_str(),
                    s.file_path,
                    s.line_number,
                    s.sink_pattern,
                    s.description
                ));
            }
        }

        if !self.archives.is_empty() {
            ctx.push_str("\nPaquetes e inspección de archivos comprimidos:\n");
            for a in self.archives.iter().take(5) {
                ctx.push_str(&format!(
                    "- {}: {} archivos (descomprimido: {} bytes)\n",
                    a.file_path, a.file_count, a.uncompressed_bytes
                ));
            }
        }

        ctx.push_str("--------------------------------------------------------------------\n");
        ctx
    }
}

/// Comprehensive 7-phase intelligent project analysis engine.
pub struct ProjectAnalysisPipeline;

impl ProjectAnalysisPipeline {
    /// Initializes default phase states for the 7 stages.
    pub fn initial_phases() -> Vec<PhaseState> {
        vec![
            PhaseState {
                index: 1,
                name: "FASE 01 — PROJECT INITIALIZATION".into(),
                description: "Identificando directorio y comprobando permisos".into(),
                status: PhaseStatus::Pending,
                detail: "".into(),
                elapsed_ms: 0,
            },
            PhaseState {
                index: 2,
                name: "FASE 02 — CONTENT SCANNING".into(),
                description: "Recorriendo estructura, extensiones y formatos".into(),
                status: PhaseStatus::Pending,
                detail: "".into(),
                elapsed_ms: 0,
            },
            PhaseState {
                index: 3,
                name: "FASE 03 — PACKAGE EXTRACTION".into(),
                description: "Inspeccionando paquetes ZIP/JAR y dependencias".into(),
                status: PhaseStatus::Pending,
                detail: "".into(),
                elapsed_ms: 0,
            },
            PhaseState {
                index: 4,
                name: "FASE 04 — OBFUSCATION DETECTION".into(),
                description: "Buscando patrones y capas de código ofuscado".into(),
                status: PhaseStatus::Pending,
                detail: "".into(),
                elapsed_ms: 0,
            },
            PhaseState {
                index: 5,
                name: "FASE 05 — CODE RECONSTRUCTION".into(),
                description: "Normalización, resolución y reconstrucción AST".into(),
                status: PhaseStatus::Pending,
                detail: "".into(),
                elapsed_ms: 0,
            },
            PhaseState {
                index: 6,
                name: "FASE 06 — SECURITY INSPECTION".into(),
                description: "Inspección defensiva estática de amenazas y sinks".into(),
                status: PhaseStatus::Pending,
                detail: "".into(),
                elapsed_ms: 0,
            },
            PhaseState {
                index: 7,
                name: "FASE 07 — ANALYSIS FINALIZATION".into(),
                description: "Consolidación de métricas y generación de informe".into(),
                status: PhaseStatus::Pending,
                detail: "".into(),
                elapsed_ms: 0,
            },
        ]
    }

    /// Executes the full 7-phase analysis on the given project path, emitting progress callbacks.
    pub fn run<F>(project_path: &Path, mut progress_cb: F) -> ProjectAnalysisReport
    where
        F: FnMut(AnalysisProgressUpdate),
    {
        let start_time = Instant::now();
        let mut phases = Self::initial_phases();
        let project_name = project_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("WORKSPACE")
            .to_string();
        let project_path_str = project_path.to_string_lossy().to_string();

        let mut all_files: Vec<PathBuf> = Vec::new();
        let mut total_dirs: usize = 0;
        let mut total_bytes: u64 = 0;
        let mut lang_counts: HashMap<String, usize> = HashMap::new();
        let mut archives_inspected: Vec<ArchiveInspection> = Vec::new();
        let mut obfuscation_findings: Vec<ObfuscationFinding> = Vec::new();
        let mut security_findings: Vec<SecurityFinding> = Vec::new();
        let mut incompatible_files: Vec<(String, String)> = Vec::new();
        let mut reconstructed_count: usize = 0;

        // -----------------------------------------------------------------
        // FASE 01 — PROJECT INITIALIZATION
        // -----------------------------------------------------------------
        {
            let p_start = Instant::now();
            phases[0].status = PhaseStatus::Running;
            phases[0].detail = "Verificando permisos y ruta base...".into();
            progress_cb(AnalysisProgressUpdate {
                current_phase: 1,
                progress_percent: 5.0,
                current_operation: "Inicializando espacio de análisis...".into(),
                files_processed: 0,
                files_total: 0,
                phases: phases.clone(),
                live_log: format!("[INIT] Inspección iniciada en: {}", project_path_str),
            });

            if !project_path.exists() {
                phases[0].status = PhaseStatus::Error;
                phases[0].detail = "Ruta no accesible o inexistente.".into();
                return Self::build_fallback_report(
                    &project_name,
                    &project_path_str,
                    "Error: La ruta del proyecto no existe.",
                );
            }

            phases[0].status = PhaseStatus::Completed;
            phases[0].detail = format!("Proyecto '{}' validado con éxito.", project_name);
            phases[0].elapsed_ms = p_start.elapsed().as_millis() as u64;
        }

        // -----------------------------------------------------------------
        // FASE 02 — CONTENT SCANNING
        // -----------------------------------------------------------------
        {
            let p_start = Instant::now();
            phases[1].status = PhaseStatus::Running;
            phases[1].detail = "Analizando contenido del proyecto...".into();
            progress_cb(AnalysisProgressUpdate {
                current_phase: 2,
                progress_percent: 15.0,
                current_operation: "Recorriendo estructura de directorios...".into(),
                files_processed: 0,
                files_total: 0,
                phases: phases.clone(),
                live_log: "[SCAN] Enumerando archivos y catalogando extensiones...".into(),
            });

            Self::walk_directory_recursive(
                project_path,
                0,
                &mut all_files,
                &mut total_dirs,
                &mut total_bytes,
                &mut lang_counts,
            );

            phases[1].status = PhaseStatus::Completed;
            phases[1].detail = format!(
                "{} archivos, {} carpetas ({:.2} MB).",
                all_files.len(),
                total_dirs,
                total_bytes as f64 / (1024.0 * 1024.0)
            );
            phases[1].elapsed_ms = p_start.elapsed().as_millis() as u64;
        }

        let total_files = all_files.len();

        // -----------------------------------------------------------------
        // FASE 03 — PACKAGE EXTRACTION (ZIP / JAR)
        // -----------------------------------------------------------------
        {
            let p_start = Instant::now();
            phases[2].status = PhaseStatus::Running;
            phases[2].detail = "Inspeccionando paquetes y dependencias...".into();
            progress_cb(AnalysisProgressUpdate {
                current_phase: 3,
                progress_percent: 30.0,
                current_operation: "Buscando paquetes JAR y ZIP...".into(),
                files_processed: 0,
                files_total: total_files,
                phases: phases.clone(),
                live_log: "[ARCHIVE] Comprobando archivos comprimidos con salvaguardas anti-zipbomb...".into(),
            });

            let archive_files: Vec<PathBuf> = all_files
                .iter()
                .filter(|p| {
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                    ext == "jar" || ext == "zip" || ext == "war"
                })
                .cloned()
                .collect();

            if archive_files.is_empty() {
                phases[2].status = PhaseStatus::Skipped;
                phases[2].detail = "Omitida: No se encontraron paquetes JAR o ZIP en la raíz.".into();
            } else {
                for arch_path in &archive_files {
                    if let Ok(inspection) = Self::safely_inspect_archive(arch_path, project_path) {
                        archives_inspected.push(inspection);
                    }
                }
                phases[2].status = PhaseStatus::Completed;
                phases[2].detail = format!(
                    "{} paquetes inspeccionados con éxito.",
                    archives_inspected.len()
                );
            }
            phases[2].elapsed_ms = p_start.elapsed().as_millis() as u64;
        }

        // -----------------------------------------------------------------
        // FASE 04 — OBFUSCATION DETECTION
        // -----------------------------------------------------------------
        {
            let p_start = Instant::now();
            phases[3].status = PhaseStatus::Running;
            phases[3].detail = "Buscando código ofuscado...".into();
            progress_cb(AnalysisProgressUpdate {
                current_phase: 4,
                progress_percent: 50.0,
                current_operation: "Analizando identificadores, entropía y cadenas codificadas...".into(),
                files_processed: 0,
                files_total: total_files,
                phases: phases.clone(),
                live_log: "[OBFUSCATION] Ejecutando análisis heurístico de capas de codificación...".into(),
            });

            let mut processed = 0;
            for file_path in &all_files {
                let rel = file_path
                    .strip_prefix(project_path)
                    .unwrap_or(file_path)
                    .to_string_lossy()
                    .to_string();

                if Self::is_inspectable_code_file(file_path) {
                    if let Ok(bytes) = std::fs::read(file_path) {
                        let content = String::from_utf8_lossy(&bytes);
                        let findings = Self::detect_obfuscation_in_source(&rel, &content);
                        obfuscation_findings.extend(findings);
                    } else {
                        incompatible_files.push((rel.clone(), "Archivo no legible o sin permisos".into()));
                    }
                }
                processed += 1;
                if processed % 15 == 0 || processed == total_files {
                    let pct = 50.0 + (processed as f32 / total_files.max(1) as f32) * 20.0;
                    progress_cb(AnalysisProgressUpdate {
                        current_phase: 4,
                        progress_percent: pct,
                        current_operation: format!("Inspeccionando: {}", rel),
                        files_processed: processed,
                        files_total: total_files,
                        phases: phases.clone(),
                        live_log: format!("[OBFUSCATION] {}", rel),
                    });
                }
            }

            if obfuscation_findings.is_empty() {
                phases[3].status = PhaseStatus::Completed;
                phases[3].detail = "Sin ofuscación relevante detectada.".into();
            } else {
                let obf_files: HashSet<&str> = obfuscation_findings
                    .iter()
                    .map(|f| f.file_path.as_str())
                    .collect();
                phases[3].status = PhaseStatus::Warning;
                phases[3].detail = format!(
                    "¡ALERTA! Detectadas {} evidencias de ofuscación en {} archivos.",
                    obfuscation_findings.len(),
                    obf_files.len()
                );
            }
            phases[3].elapsed_ms = p_start.elapsed().as_millis() as u64;
        }

        // -----------------------------------------------------------------
        // FASE 05 — CODE RECONSTRUCTION
        // -----------------------------------------------------------------
        {
            let p_start = Instant::now();
            phases[4].status = PhaseStatus::Running;
            phases[4].detail = "Analizando estructuras y reconstruyendo código...".into();
            progress_cb(AnalysisProgressUpdate {
                current_phase: 5,
                progress_percent: 75.0,
                current_operation: "Resolviendo pasadas de reconstrucción AST...".into(),
                files_processed: total_files,
                files_total: total_files,
                phases: phases.clone(),
                live_log: "[RECONSTRUCTION] Evaluando compatibilidad con el motor de desofuscación...".into(),
            });

            if obfuscation_findings.is_empty() {
                phases[4].status = PhaseStatus::Skipped;
                phases[4].detail = "No necesaria: no se han detectado estructuras ofuscadas compatibles.".into();
            } else {
                // Reconstruir los archivos compatibles sin modificar jamás los originales
                let mut recons = 0;
                let target_files: HashSet<String> = obfuscation_findings
                    .iter()
                    .map(|f| f.file_path.clone())
                    .collect();

                for rel_path in &target_files {
                    let full = project_path.join(rel_path);
                    if let Ok(bytes) = std::fs::read(&full) {
                        let src = String::from_utf8_lossy(&bytes);
                        let res = DeobfuscatorEngine::process(&src, &DeobfuscationConfig::default());
                        if res.strings_decoded > 0 || res.variables_renamed > 0 || res.dead_branches_pruned > 0 || res.code != src {
                            recons += 1;
                        }
                    }
                }
                reconstructed_count = recons;

                phases[4].status = PhaseStatus::Completed;
                if reconstructed_count > 0 {
                    phases[4].detail = format!(
                        "Reconstruidos {} archivos compatibles en memoria.",
                        reconstructed_count
                    );
                } else {
                    phases[4].detail = format!(
                        "Identificados {} archivos ofuscados. Tablas de mapeo léxico y símbolos preparadas para KRONOS AI.",
                        target_files.len()
                    );
                }
            }
            phases[4].elapsed_ms = p_start.elapsed().as_millis() as u64;
        }

        // -----------------------------------------------------------------
        // FASE 06 — SECURITY INSPECTION
        // -----------------------------------------------------------------
        {
            let p_start = Instant::now();
            phases[5].status = PhaseStatus::Running;
            phases[5].detail = "Inspeccionando posibles amenazas de seguridad...".into();
            progress_cb(AnalysisProgressUpdate {
                current_phase: 6,
                progress_percent: 88.0,
                current_operation: "Escaneando sinks peligrosos y llamadas a procesos/red...".into(),
                files_processed: total_files,
                files_total: total_files,
                phases: phases.clone(),
                live_log: "[SECURITY] Ejecutando análisis defensivo estático Zero-Exec...".into(),
            });

            for file_path in &all_files {
                let rel = file_path
                    .strip_prefix(project_path)
                    .unwrap_or(file_path)
                    .to_string_lossy()
                    .to_string();

                if Self::is_inspectable_code_file(file_path) {
                    if let Ok(bytes) = std::fs::read(file_path) {
                        let content = String::from_utf8_lossy(&bytes);
                        let sec_findings = Self::audit_security_sinks(&rel, &content);
                        security_findings.extend(sec_findings);
                    }
                }
            }

            let has_threats = security_findings
                .iter()
                .any(|s| s.classification == SecurityClassification::ConfirmedThreat);

            if has_threats {
                phases[5].status = PhaseStatus::Warning;
                phases[5].detail = format!(
                    "Detectadas {} alertas de seguridad críticas.",
                    security_findings.len()
                );
            } else if !security_findings.is_empty() {
                phases[5].status = PhaseStatus::Completed;
                phases[5].detail = format!(
                    "Identificados {} indicadores sospechosos o sinks de proceso.",
                    security_findings.len()
                );
            } else {
                phases[5].status = PhaseStatus::Completed;
                phases[5].detail = "Sin indicadores de malware ni sinks críticos detectados.".into();
            }
            phases[5].elapsed_ms = p_start.elapsed().as_millis() as u64;
        }

        // -----------------------------------------------------------------
        // FASE 07 — ANALYSIS FINALIZATION
        // -----------------------------------------------------------------
        {
            let p_start = Instant::now();
            phases[6].status = PhaseStatus::Running;
            phases[6].detail = "Finalizando análisis del proyecto...".into();
            progress_cb(AnalysisProgressUpdate {
                current_phase: 7,
                progress_percent: 98.0,
                current_operation: "Consolidando métricas e índice técnico...".into(),
                files_processed: total_files,
                files_total: total_files,
                phases: phases.clone(),
                live_log: "[FINAL] Consolidando resultados y preparando informe adaptativo...".into(),
            });

            phases[6].status = PhaseStatus::Completed;
            phases[6].detail = "Métricas y resumen técnico consolidados.".into();
            phases[6].elapsed_ms = p_start.elapsed().as_millis() as u64;
        }

        let total_elapsed_ms = start_time.elapsed().as_millis() as u64;

        // Determinar caso de veredicto inteligente
        let (verdict_case, verdict_headline, verdict_summary) = Self::evaluate_verdict(
            &obfuscation_findings,
            &security_findings,
            &incompatible_files,
            total_files,
            reconstructed_count,
        );

        let mut lang_dist: Vec<(String, usize)> = lang_counts.into_iter().collect();
        lang_dist.sort_by_key(|b| std::cmp::Reverse(b.1));

        let report = ProjectAnalysisReport {
            project_name,
            project_path: project_path_str,
            total_files,
            total_dirs,
            total_bytes,
            total_elapsed_ms,
            language_distribution: lang_dist,
            archives: archives_inspected,
            obfuscation_findings,
            security_findings,
            incompatible_files,
            reconstructed_count,
            verdict_case,
            verdict_headline,
            verdict_summary,
            markdown_report: String::new(),
        };

        progress_cb(AnalysisProgressUpdate {
            current_phase: 7,
            progress_percent: 100.0,
            current_operation: "Análisis completado.".into(),
            files_processed: total_files,
            files_total: total_files,
            phases,
            live_log: format!("[DONE] Veredicto final: {}", report.verdict_headline),
        });

        report
    }

    /// Evaluates the smart outcome case (Case A, B, C or D).
    fn evaluate_verdict(
        obf: &[ObfuscationFinding],
        sec: &[SecurityFinding],
        incompat: &[(String, String)],
        total_files: usize,
        recons_count: usize,
    ) -> (AnalysisVerdictCase, String, String) {
        let has_confirmed_threat = sec
            .iter()
            .any(|s| s.classification == SecurityClassification::ConfirmedThreat);

        if has_confirmed_threat {
            (
                AnalysisVerdictCase::CaseD_SecurityThreat,
                "ALERTA: Se han detectado posibles amenazas o patrones de persistencia".into(),
                format!(
                    "El análisis defensivo estático ha identificado {} hallazgos de seguridad críticos. Se recomienda no ejecutar el binario y revisar los archivos señalados.",
                    sec.len()
                ),
            )
        } else if !obf.is_empty() {
            let obf_files: HashSet<&str> = obf.iter().map(|f| f.file_path.as_str()).collect();
            let has_allatori = obf.iter().any(|f| {
                f.pattern_name.contains("Griego")
                    || f.pattern_name.contains("Allatori")
                    || f.pattern_name.contains("Unicode")
            });
            let headline = format!(
                "ALERTA: Código Ofuscado Detectado ({} archivos afectados)",
                obf_files.len()
            );
            let summary = if has_allatori {
                format!(
                    "Se han detectado {} evidencias de ofuscación severa (nomenclatura en alfabeto griego/Unicode Allatori/Zelix, tablas de cadenas cifradas y trampas de control de flujo) en {} archivos. KRONOS AI y las herramientas de inspección inversa están preparadas para el análisis.",
                    obf.len(),
                    obf_files.len()
                )
            } else {
                format!(
                    "Se han detectado {} evidencias de ofuscación (cadenas cifradas, entropía léxica o flujo alterado) en {} archivos. Reconstrucción disponible para {} archivo(s).",
                    obf.len(),
                    obf_files.len(),
                    recons_count
                )
            };
            (
                AnalysisVerdictCase::CaseA_Obfuscated,
                headline,
                summary,
            )
        } else if incompat.len() > total_files.saturating_div(2) && total_files > 0 {
            (
                AnalysisVerdictCase::CaseC_IncompatibleFiles,
                "Archivos No Compatibles con Desofuscación Estática".into(),
                format!(
                    "El proyecto contiene {} archivos binarios o con formatos propietarios que no pueden ser desofuscados mediante el motor AST estándar.",
                    incompat.len()
                ),
            )
        } else {
            (
                AnalysisVerdictCase::CaseB_CleanProject,
                "Análisis completado. No se han detectado patrones de ofuscación relevantes".into(),
                "El código fuente y las clases analizadas presentan estructuras idiomáticas limpias, identificadores legibles y ausencia de trampas léxicas o de flujo.".into(),
            )
        }
    }

    /// Recursively counts and enumerates directory contents with max depth protection.
    fn walk_directory_recursive(
        dir: &Path,
        depth: usize,
        out_files: &mut Vec<PathBuf>,
        out_dirs: &mut usize,
        out_bytes: &mut u64,
        lang_counts: &mut HashMap<String, usize>,
    ) {
        if depth > 12 || out_files.len() >= 5000 {
            return;
        }

        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let fname = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };

            // Ignorar carpetas ocultas y dependencias de build masivas
            if fname.starts_with('.')
                || fname.eq_ignore_ascii_case("node_modules")
                || fname.eq_ignore_ascii_case("target")
                || fname.eq_ignore_ascii_case(".git")
                || fname.eq_ignore_ascii_case("__pycache__")
                || fname.eq_ignore_ascii_case(".idea")
            {
                continue;
            }

            if path.is_dir() {
                *out_dirs += 1;
                Self::walk_directory_recursive(
                    &path,
                    depth + 1,
                    out_files,
                    out_dirs,
                    out_bytes,
                    lang_counts,
                );
            } else if path.is_file() {
                if let Ok(meta) = path.metadata() {
                    *out_bytes += meta.len();
                }
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("no_ext")
                    .to_lowercase();
                *lang_counts.entry(ext).or_insert(0) += 1;
                out_files.push(path);
            }
        }
    }

    /// Checks if a file has an extension suitable for static source inspection or contains obfuscated unicode naming.
    pub fn is_inspectable_code_file(path: &Path) -> bool {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if Self::contains_obfuscated_unicode(name) {
            return true;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        matches!(
            ext.as_str(),
            "py" | "pyw" | "java" | "rs" | "js" | "ts" | "c" | "cpp" | "yml" | "yaml" | "json" | "xml" | "class" | "txt"
        )
    }

    /// Checks if a string contains obfuscated Unicode characters (Greek, Cyrillic homoglyphs, math symbols, invisible chars).
    pub fn contains_obfuscated_unicode(s: &str) -> bool {
        s.chars().any(|c| {
            matches!(
                c,
                '\u{0370}'..='\u{03FF}' // Greek and Coptic (Allatori / Zelix classic)
                | '\u{1F00}'..='\u{1FFF}' // Greek Extended
                | '\u{0400}'..='\u{04FF}' // Cyrillic homoglyphs
                | '\u{2000}'..='\u{206F}' // Invisible spaces / zero-width joiners / general punctuation
                | '\u{2100}'..='\u{214F}' // Letterlike symbols
                | '\u{2200}'..='\u{22FF}' // Mathematical Operators
                | '\u{2E80}'..='\u{9FFF}' // CJK / illegal opcode markers
            )
        })
    }

    /// Strips leading comments, especially decompilation block comments (e.g. `/* 20 */` or `/*     */`).
    pub fn strip_leading_comment(mut s: &str) -> &str {
        s = s.trim();
        while s.starts_with("/*") {
            if let Some(end) = s.find("*/") {
                s = s[end + 2..].trim();
            } else {
                break;
            }
        }
        s
    }

    /// Safely inspects an archive file (ZIP/JAR) without extracting arbitrary files to disk.
    fn safely_inspect_archive(
        arch_path: &Path,
        root_path: &Path,
    ) -> Result<ArchiveInspection, String> {
        let file = File::open(arch_path).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

        let rel_path = arch_path
            .strip_prefix(root_path)
            .unwrap_or(arch_path)
            .to_string_lossy()
            .to_string();

        let file_count = zip.len();
        let mut uncompressed_bytes: u64 = 0;
        let mut nested_archives: Vec<String> = Vec::new();
        let mut main_class: Option<String> = None;
        let mut manifest_entries: Vec<String> = Vec::new();

        // Limitar número de entradas a 2000 para evitar zip-bombs
        let max_entries = file_count.min(2000);
        for i in 0..max_entries {
            if let Ok(mut entry) = zip.by_index(i) {
                let name = entry.name().to_string();

                // Detección anti-path traversal
                if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
                    continue;
                }

                uncompressed_bytes = uncompressed_bytes.saturating_add(entry.size());

                let lower = name.to_lowercase();
                if lower.ends_with(".jar") || lower.ends_with(".zip") {
                    nested_archives.push(name.clone());
                }

                // Leer el manifest de forma segura (máximo 64 KB)
                if name.eq_ignore_ascii_case("META-INF/MANIFEST.MF") && entry.size() < 65536 {
                    let mut buf = Vec::new();
                    if entry.read_to_end(&mut buf).is_ok() {
                        let manifest_str = String::from_utf8_lossy(&buf);
                        for line in manifest_str.lines() {
                            let trimmed = line.trim();
                            if trimmed.starts_with("Main-Class:") {
                                main_class = Some(
                                    trimmed.trim_start_matches("Main-Class:").trim().to_string(),
                                );
                            }
                            if trimmed.starts_with("Class-Path:")
                                || trimmed.starts_with("Implementation-Title:")
                            {
                                manifest_entries.push(trimmed.to_string());
                            }
                        }
                    }
                }
            }
        }

        Ok(ArchiveInspection {
            file_path: rel_path,
            file_count,
            uncompressed_bytes,
            nested_archives,
            main_class,
            manifest_entries,
        })
    }

    /// Detects real technical obfuscation patterns in source code.
    pub fn detect_obfuscation_in_source(
        file_path: &str,
        content: &str,
    ) -> Vec<ObfuscationFinding> {
        let mut findings = Vec::new();

        // 1. Detección en el nombre/ruta del archivo (Nomenclatura ofuscada Allatori / Zelix)
        if Self::contains_obfuscated_unicode(file_path) {
            findings.push(ObfuscationFinding {
                file_path: file_path.to_string(),
                line_number: 1,
                pattern_name: "Nomenclatura Ofuscada (Alfabeto Griego / Allatori / Zelix)".into(),
                evidence: file_path.chars().take(80).collect(),
                severity: "CRÍTICA".into(),
            });
        }

        // 2. Detección línea por línea en el contenido del código
        for (idx, raw_line) in content.lines().enumerate() {
            let line_num = idx + 1;
            let trimmed = raw_line.trim();

            // Detección de artefactos de descompilación de bytecode antes de ignorar comentarios
            if (trimmed.contains("// Byte code:")
                || trimmed.contains("// Bytecode:")
                || trimmed.contains("<illegal opcode>"))
                && findings.len() < 40
            {
                findings.push(ObfuscationFinding {
                    file_path: file_path.to_string(),
                    line_number: line_num,
                    pattern_name: "Artefacto de Descompilación de Bytecode Ofuscado".into(),
                    evidence: trimmed.chars().take(70).collect(),
                    severity: "ALTA".into(),
                });
            }

            // Desempaquetar comentarios iniciales de descompilador (/* 20 */, /*     */)
            let code = Self::strip_leading_comment(trimmed);
            if code.is_empty() || code.starts_with('#') || code.starts_with("//") {
                continue;
            }

            // Detección de identificadores en alfabeto griego o caracteres no-ASCII en código
            if Self::contains_obfuscated_unicode(code) && findings.len() < 40 {
                findings.push(ObfuscationFinding {
                    file_path: file_path.to_string(),
                    line_number: line_num,
                    pattern_name: "Identificador Ofuscado (Alfabeto Griego / Símbolos Unicode)".into(),
                    evidence: code.chars().take(70).collect(),
                    severity: "ALTA".into(),
                });
            }

            // Detección de trucos de flujo / constantes opacas de Allatori (Integer.reverse)
            if (code.contains("Integer.reverse(-2147483648)") || code.contains("Integer.reverse("))
                && findings.len() < 40
            {
                findings.push(ObfuscationFinding {
                    file_path: file_path.to_string(),
                    line_number: line_num,
                    pattern_name: "Predicado Opaco / Constante de Control de Flujo (Allatori)".into(),
                    evidence: code.chars().take(70).collect(),
                    severity: "ALTA".into(),
                });
            }

            // Detección de matrices y tablas de cadenas cifradas (String[] a, String[] b)
            if code.contains("private static String[] a;")
                || code.contains("private static String[] b;")
                || code.contains("private static final String[]")
            {
                findings.push(ObfuscationFinding {
                    file_path: file_path.to_string(),
                    line_number: line_num,
                    pattern_name: "Tabla de Cadenas Cifradas (String Encryption Matrix)".into(),
                    evidence: code.chars().take(70).collect(),
                    severity: "CRÍTICA".into(),
                });
            }

            // Detección de chr() concatenados repetitivos
            if code.matches("chr(").count() >= 3 {
                findings.push(ObfuscationFinding {
                    file_path: file_path.to_string(),
                    line_number: line_num,
                    pattern_name: "Secuencia de chr() concatenada".into(),
                    evidence: code.chars().take(70).collect(),
                    severity: "MEDIA".into(),
                });
            }

            // Detección de decodificadores Hex / Base64 en línea
            if code.contains("bytes.fromhex(")
                || code.contains("base64.b64decode(")
                || code.contains("b64decode(")
            {
                findings.push(ObfuscationFinding {
                    file_path: file_path.to_string(),
                    line_number: line_num,
                    pattern_name: "Decodificador estático Base64/Hex".into(),
                    evidence: code.chars().take(70).collect(),
                    severity: "ALTA".into(),
                });
            }

            // Detección de identificadores ofuscados hexadecimales (_0x... / _Il...)
            if code.contains("_0x") || code.contains("_Il") || code.contains("_O0") {
                findings.push(ObfuscationFinding {
                    file_path: file_path.to_string(),
                    line_number: line_num,
                    pattern_name: "Identificadores Hexadecimales Evasivos".into(),
                    evidence: code.chars().take(70).collect(),
                    severity: "MEDIA".into(),
                });
            }

            // Detección de ramas opacas o código muerto intencionado
            if code.starts_with("if False:") || code.starts_with("if 0:") {
                findings.push(ObfuscationFinding {
                    file_path: file_path.to_string(),
                    line_number: line_num,
                    pattern_name: "Predicado Opaco / Rama Muerta Inalcanzable".into(),
                    evidence: code.chars().take(70).collect(),
                    severity: "BAJA".into(),
                });
            }

            // Detección de proxies de carga dinámica en Java
            if code.contains("ClassLoader")
                && (code.contains("defineClass") || code.contains("findClass"))
            {
                findings.push(ObfuscationFinding {
                    file_path: file_path.to_string(),
                    line_number: line_num,
                    pattern_name: "Cargador Dinámico de Clases / Reflexión Oculta".into(),
                    evidence: code.chars().take(70).collect(),
                    severity: "ALTA".into(),
                });
            }
        }

        findings
    }

    /// Defensively audits dangerous execution sinks and process spawning.
    pub fn audit_security_sinks(file_path: &str, content: &str) -> Vec<SecurityFinding> {
        let mut findings = Vec::new();

        let sinks: &[(&str, SecurityClassification, &str, &str)] = &[
            (
                "Runtime.getRuntime().exec",
                SecurityClassification::PotentiallyDangerousCall,
                "Ejecución de procesos del sistema operativo vía Runtime",
                "Verificar que los comandos no provengan de entradas no sanitizadas.",
            ),
            (
                "ProcessBuilder",
                SecurityClassification::PotentiallyDangerousCall,
                "Construcción y ejecución de procesos nativos",
                "Comprobar parámetros y comandos ejecutados.",
            ),
            (
                "os.system(",
                SecurityClassification::ConfirmedThreat,
                "Invocación de comando de shell sin aislamiento",
                "Sustituir por APIs defensivas seguras sin intérprete de comandos.",
            ),
            (
                "subprocess.Popen",
                SecurityClassification::PotentiallyDangerousCall,
                "Creación de subprocesos externos",
                "Evitar el flag shell=True para prevenir inyección de comandos.",
            ),
            (
                "eval(",
                SecurityClassification::ConfirmedThreat,
                "Ejecución dinámica de código arbitrario",
                "Eliminar o refactorizar a parsing declarativo AST.",
            ),
            (
                "exec(",
                SecurityClassification::ConfirmedThreat,
                "Ejecución dinámica de bloques de código",
                "Prohibir ejecución dinámica en código de producción.",
            ),
            (
                "URLClassLoader",
                SecurityClassification::SuspiciousIndicator,
                "Carga de clases remotas desde URLs de red",
                "Validar procedencia de JARs y verificar firmas criptográficas.",
            ),
        ];

        for (idx, line) in content.lines().enumerate() {
            let code = Self::strip_leading_comment(line.trim());
            if code.is_empty() || code.starts_with('#') || code.starts_with("//") {
                continue;
            }

            for &(pattern, classification, desc, rec) in sinks {
                if code.contains(pattern) {
                    // Evitar que llamadas de método Java (.exec) se confundan con la función exec() de Python
                    if pattern == "exec(" && code.contains(".exec(") {
                        continue;
                    }

                    findings.push(SecurityFinding {
                        file_path: file_path.to_string(),
                        line_number: idx + 1,
                        classification,
                        sink_pattern: pattern.to_string(),
                        description: desc.to_string(),
                        confidence: "Alta (Firma exacta de sink)".into(),
                        recommendation: rec.to_string(),
                    });
                }
            }
        }

        findings
    }

    /// Fallback report generator for unreadable or invalid paths.
    fn build_fallback_report(
        name: &str,
        path: &str,
        err_msg: &str,
    ) -> ProjectAnalysisReport {
        ProjectAnalysisReport {
            project_name: name.to_string(),
            project_path: path.to_string(),
            total_files: 0,
            total_dirs: 0,
            total_bytes: 0,
            total_elapsed_ms: 0,
            language_distribution: Vec::new(),
            archives: Vec::new(),
            obfuscation_findings: Vec::new(),
            security_findings: Vec::new(),
            incompatible_files: Vec::new(),
            reconstructed_count: 0,
            verdict_case: AnalysisVerdictCase::CaseC_IncompatibleFiles,
            verdict_headline: "Error en la Inicialización del Proyecto".into(),
            verdict_summary: err_msg.into(),
            markdown_report: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_initial_phases_count() {
        let phases = ProjectAnalysisPipeline::initial_phases();
        assert_eq!(phases.len(), 7);
        assert_eq!(phases[0].index, 1);
        assert_eq!(phases[6].index, 7);
        assert_eq!(phases[0].status, PhaseStatus::Pending);
    }

    #[test]
    fn test_detect_obfuscation_finds_chr_and_hex() {
        let sample = "var_1 = chr(104) + chr(101) + chr(108)\n_0x1a = bytes.fromhex('4142')\nif False:\n    print(1)";
        let findings = ProjectAnalysisPipeline::detect_obfuscation_in_source("sample.py", sample);
        assert!(findings.len() >= 3);
        assert!(findings.iter().any(|f| f.pattern_name.contains("chr")));
        assert!(findings.iter().any(|f| f.pattern_name.contains("Hex")));
        assert!(findings.iter().any(|f| f.pattern_name.contains("Opaco")));
    }

    #[test]
    fn test_audit_security_sinks_detects_eval_and_runtime() {
        let java_sample = "Runtime.getRuntime().exec(\"calc.exe\");\nProcessBuilder pb = new ProcessBuilder();";
        let findings = ProjectAnalysisPipeline::audit_security_sinks("App.java", java_sample);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].classification, SecurityClassification::PotentiallyDangerousCall);

        let py_sample = "eval('__import__(\"os\").system(\"dir\")')";
        let py_findings = ProjectAnalysisPipeline::audit_security_sinks("exploit.py", py_sample);
        assert_eq!(py_findings.len(), 1);
        assert_eq!(py_findings[0].classification, SecurityClassification::ConfirmedThreat);
    }

    #[test]
    fn test_evaluate_verdict_clean_project() {
        let (case, headline, _) = ProjectAnalysisPipeline::evaluate_verdict(&[], &[], &[], 10, 0);
        assert_eq!(case, AnalysisVerdictCase::CaseB_CleanProject);
        assert!(headline.contains("No se han detectado patrones"));
    }

    #[test]
    fn test_evaluate_verdict_obfuscated_project() {
        let findings = vec![ObfuscationFinding {
            file_path: "app.py".into(),
            line_number: 1,
            pattern_name: "Hex".into(),
            evidence: "_0x1 = 2".into(),
            severity: "ALTA".into(),
        }];
        let (case, headline, _) = ProjectAnalysisPipeline::evaluate_verdict(&findings, &[], &[], 5, 1);
        assert_eq!(case, AnalysisVerdictCase::CaseA_Obfuscated);
        assert!(headline.contains("Código Ofuscado Detectado"));
    }

    #[test]
    fn test_strip_leading_comment() {
        assert_eq!(
            ProjectAnalysisPipeline::strip_leading_comment("/* 20 */ public class Main {"),
            "public class Main {"
        );
        assert_eq!(
            ProjectAnalysisPipeline::strip_leading_comment("/*     */   int x = 1;"),
            "int x = 1;"
        );
        assert_eq!(
            ProjectAnalysisPipeline::strip_leading_comment("/* 1 */ /* 2 */ int y = 2;"),
            "int y = 2;"
        );
    }

    #[test]
    fn test_detect_obfuscation_greek_unicode_and_allatori() {
        let decompiled_java = r#"
/*     */ package com.nickuc.login;
/*  20 */ public class αΓασΣξηΛππ implements ζζΦΣΛθΨΔμπυ {
/*     */   private static int a = Integer.reverse(-2147483648);
/*     */   private static String[] a;
/*     */   // Byte code:
/*     */   //   16: <illegal opcode> 㺀 : (IJ)Ljava/lang/String;
/*     */ }
"#;
        let findings = ProjectAnalysisPipeline::detect_obfuscation_in_source("αΓασΣξηΛππ.java", decompiled_java);
        assert!(!findings.is_empty());
        assert!(findings.iter().any(|f| f.pattern_name.contains("Griego")));
        assert!(findings.iter().any(|f| f.pattern_name.contains("Allatori")));
        assert!(findings.iter().any(|f| f.pattern_name.contains("Bytecode")));
        assert!(findings.iter().any(|f| f.pattern_name.contains("Tabla de Cadenas")));
    }

    #[test]
    fn test_pipeline_on_real_nlogin_folder() {
        let p = std::path::Path::new(r"C:\Users\Igor2\Desktop\nLoginpruebas.jar.src");
        if !p.exists() {
            return;
        }
        let report = ProjectAnalysisPipeline::run(p, |_| {});
        println!("Verdict case: {:?}", report.verdict_case);
        println!("Headline: {}", report.verdict_headline);
        println!("Obf findings: {}", report.obfuscation_findings.len());
        println!("Sec findings: {}", report.security_findings.len());
        assert_eq!(report.verdict_case, AnalysisVerdictCase::CaseA_Obfuscated);
        assert!(report.obfuscation_findings.len() >= 700);
        assert!(report.verdict_headline.contains("ALERTA: Código Ofuscado Detectado"));
    }
}
