use aegis_core::{
    archive::ComplianceLevel,
    extract::{ExtractionResult, Extractor},
    patch::{PatchApplier, PatchRecipe},
    AegisCore,
};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use std::path::PathBuf;

/// Python wrapper for AegisCore
#[pyclass(name = "AegisCore")]
struct PyAegisCore {
    inner: AegisCore,
}

#[pymethods]
impl PyAegisCore {
    #[new]
    fn new() -> PyResult<Self> {
        let core = AegisCore::new()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        Ok(Self { inner: core })
    }

    /// Load compliance profiles from directory
    fn load_compliance_profiles(&mut self, profiles_dir: &str) -> PyResult<()> {
        let path = PathBuf::from(profiles_dir);
        self.inner
            .load_compliance_profiles(&path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Create an extractor instance
    fn create_extractor(&self) -> PyResult<PyExtractor> {
        let extractor = self.inner.create_extractor();
        Ok(PyExtractor { inner: extractor })
    }

    /// Get system information
    fn system_info<'py>(&self, py: Python<'py>) -> PyResult<&'py PyDict> {
        let info = self.inner.system_info();

        let dict = PyDict::new(py);
        dict.set_item("version", &info.version)?;
        dict.set_item("git_hash", &info.git_hash)?;
        dict.set_item("registered_plugins", info.registered_plugins)?;
        dict.set_item("compliance_profiles", info.compliance_profiles)?;
        Ok(dict)
    }
}

/// Python wrapper for Extractor
#[pyclass(name = "Extractor")]
struct PyExtractor {
    inner: Extractor,
}

#[pymethods]
impl PyExtractor {
    /// Extract assets from a file
    fn extract_from_file(
        &mut self,
        source_path: &str,
        output_dir: &str,
    ) -> PyResult<PyExtractionResult> {
        let source = PathBuf::from(source_path);
        let output = PathBuf::from(output_dir);

        let result = self
            .inner
            .extract_from_file(&source, &output)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        Ok(PyExtractionResult { inner: result })
    }

    /// Extract from multiple files
    fn extract_batch(
        &mut self,
        source_paths: Vec<&str>,
        output_dir: &str,
    ) -> PyResult<Vec<PyExtractionResult>> {
        let sources: Vec<PathBuf> = source_paths.iter().map(|p| PathBuf::from(p)).collect();
        let source_refs: Vec<&std::path::Path> = sources.iter().map(|p| p.as_path()).collect();
        let output = PathBuf::from(output_dir);

        let results = self
            .inner
            .extract_batch(source_refs, &output)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        Ok(results
            .into_iter()
            .map(|r| PyExtractionResult { inner: r })
            .collect())
    }
}

/// Python wrapper for ExtractionResult
#[pyclass(name = "ExtractionResult")]
struct PyExtractionResult {
    inner: ExtractionResult,
}

#[pymethods]
impl PyExtractionResult {
    /// Get source path
    #[getter]
    fn source_path(&self) -> String {
        self.inner.source_path.to_string_lossy().to_string()
    }

    /// Get number of extracted resources
    #[getter]
    fn resource_count(&self) -> usize {
        self.inner.resources.len()
    }

    /// Get extraction warnings
    #[getter]
    fn warnings(&self) -> Vec<String> {
        self.inner.warnings.clone()
    }

    /// Get compliance information
    #[getter]
    fn compliance_info<'py>(&self, py: Python<'py>) -> PyResult<&'py PyDict> {
        let dict = PyDict::new(py);
        dict.set_item("is_compliant", self.inner.compliance_info.is_compliant)?;
        dict.set_item(
            "risk_level",
            compliance_level_to_string(self.inner.compliance_info.risk_level),
        )?;
        dict.set_item("warnings", &self.inner.compliance_info.warnings)?;
        dict.set_item(
            "recommendations",
            &self.inner.compliance_info.recommendations,
        )?;
        Ok(dict)
    }

    /// Get performance metrics
    #[getter]
    fn metrics<'py>(&self, py: Python<'py>) -> PyResult<&'py PyDict> {
        let dict = PyDict::new(py);
        dict.set_item("duration_ms", self.inner.metrics.duration_ms)?;
        dict.set_item("peak_memory_mb", self.inner.metrics.peak_memory_mb)?;
        dict.set_item("files_processed", self.inner.metrics.files_processed)?;
        dict.set_item("bytes_extracted", self.inner.metrics.bytes_extracted)?;
        Ok(dict)
    }

    /// Get resource information
    fn get_resources<'py>(&self, py: Python<'py>) -> PyResult<Vec<Py<PyDict>>> {
        let mut resources = Vec::with_capacity(self.inner.resources.len());

        for resource in &self.inner.resources {
            let dict = PyDict::new(py);
            dict.set_item("name", resource.name())?;
            dict.set_item("type", resource.resource_type())?;
            dict.set_item("estimated_memory", resource.estimated_memory_usage())?;
            resources.push(dict.into());
        }

        Ok(resources)
    }
}

/// Python wrapper for PatchRecipe
#[pyclass(name = "PatchRecipe")]
struct PyPatchRecipe {
    inner: PatchRecipe,
}

#[pymethods]
impl PyPatchRecipe {
    /// Load recipe from file
    #[staticmethod]
    fn load_from_file(path: &str) -> PyResult<Self> {
        let recipe = PatchRecipe::load_from_file(&PathBuf::from(path))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        Ok(Self { inner: recipe })
    }

    /// Save recipe to file
    fn save_to_file(&self, path: &str) -> PyResult<()> {
        self.inner
            .save_to_file(&PathBuf::from(path))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    /// Get recipe summary
    fn summary<'py>(&self, py: Python<'py>) -> PyResult<&'py PyDict> {
        let summary = self.inner.summary();

        let dict = PyDict::new(py);
        dict.set_item("version", &summary.version)?;
        dict.set_item("source_hash", &summary.source_hash)?;
        dict.set_item("asset_count", summary.asset_count)?;
        dict.set_item("total_output_size", summary.total_output_size)?;
        dict.set_item("created_at", summary.created_at.to_rfc3339())?;
        Ok(dict)
    }
}

/// Python wrapper for PatchApplier
#[pyclass(name = "PatchApplier")]
struct PyPatchApplier {
    inner: PatchApplier,
}

#[pymethods]
impl PyPatchApplier {
    /// Create from recipe
    #[new]
    fn new(recipe: &PyPatchRecipe) -> Self {
        let applier = PatchApplier::new(recipe.inner.clone());
        Self { inner: applier }
    }

    /// Load from recipe file
    #[staticmethod]
    fn from_file(recipe_path: &str) -> PyResult<Self> {
        let applier = PatchApplier::from_file(&PathBuf::from(recipe_path))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        Ok(Self { inner: applier })
    }

    /// Apply recipe to recreate assets
    fn apply<'py>(
        &self,
        py: Python<'py>,
        source_file: &str,
        output_dir: &str,
    ) -> PyResult<&'py PyDict> {
        let result = self
            .inner
            .apply(&PathBuf::from(source_file), &PathBuf::from(output_dir))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        let dict = PyDict::new(py);

        let files: PyResult<Vec<Py<PyDict>>> = result
            .applied_files
            .iter()
            .map(|f| {
                let file_dict = PyDict::new(py);
                file_dict.set_item("path", f.path.to_string_lossy().to_string())?;
                file_dict.set_item("size_bytes", f.size_bytes)?;
                file_dict.set_item("source_delta", &f.source_delta)?;
                Ok(file_dict.into())
            })
            .collect();

        dict.set_item("applied_files", files?)?;
        dict.set_item("warnings", result.warnings)?;
        dict.set_item("duration_ms", result.duration_ms)?;
        dict.set_item("recipe_version", result.recipe_version)?;

        Ok(dict)
    }
}

/// Helper function to convert ComplianceLevel to string
fn compliance_level_to_string(level: ComplianceLevel) -> &'static str {
    match level {
        ComplianceLevel::Permissive => "permissive",
        ComplianceLevel::Neutral => "neutral",
        ComplianceLevel::HighRisk => "high_risk",
    }
}

/// Convenience functions
#[pyfunction]
fn extract_assets(
    source_path: &str,
    output_dir: &str,
    compliance_profiles_dir: Option<&str>,
) -> PyResult<PyExtractionResult> {
    let mut core = AegisCore::new()
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    // Load compliance profiles if provided
    if let Some(profiles_dir) = compliance_profiles_dir {
        core.load_compliance_profiles(&PathBuf::from(profiles_dir))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
    }

    let mut extractor = core.create_extractor();
    let source = PathBuf::from(source_path);
    let output = PathBuf::from(output_dir);

    let result = extractor
        .extract_from_file(&source, &output)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

    Ok(PyExtractionResult { inner: result })
}

#[pyfunction]
fn apply_patch_recipe<'py>(
    py: Python<'py>,
    recipe_path: &str,
    source_file: &str,
    output_dir: &str,
) -> PyResult<&'py PyDict> {
    let applier = PyPatchApplier::from_file(recipe_path)?;
    applier.apply(py, source_file, output_dir)
}

#[pyfunction]
fn get_version() -> &'static str {
    aegis_core::VERSION
}

/// Python module definition
#[pymodule]
fn aegis_assets(_py: Python, m: &PyModule) -> PyResult<()> {
    // Classes
    m.add_class::<PyAegisCore>()?;
    m.add_class::<PyExtractor>()?;
    m.add_class::<PyExtractionResult>()?;
    m.add_class::<PyPatchRecipe>()?;
    m.add_class::<PyPatchApplier>()?;

    // Functions
    m.add_function(wrap_pyfunction!(extract_assets, m)?)?;
    m.add_function(wrap_pyfunction!(apply_patch_recipe, m)?)?;
    m.add_function(wrap_pyfunction!(get_version, m)?)?;

    // Module metadata
    m.add("__version__", aegis_core::VERSION)?;
    m.add(
        "__doc__",
        "Compliance-first platform for game asset extraction, preservation, and creative workflows",
    )?;

    Ok(())
}
