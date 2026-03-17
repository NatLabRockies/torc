//! Integration tests for ExecutionConfig functionality.
//!
//! Tests cover:
//! - Config parsing and serialization (YAML, JSON)
//! - Mode detection (direct, slurm, auto)
//! - Backward compatibility with legacy SlurmConfig
//! - Validation of srun_termination_signal vs sigkill_headroom_seconds
//! - Default values and helper methods

mod common;

use common::{ServerProcess, start_server};
use rstest::rstest;
use serial_test::serial;
use std::collections::HashMap;
use std::fs;
use tempfile::NamedTempFile;
use torc::client::default_api;
use torc::client::workflow_spec::{
    ExecutionConfig, ExecutionMode, StdioConfig, StdioMode, WorkflowSpec,
};

// =============================================================================
// ExecutionMode parsing tests
// =============================================================================

#[test]
fn test_execution_mode_direct_parsing() {
    let yaml = r#"
        mode: direct
    "#;
    let config: ExecutionConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.mode, ExecutionMode::Direct);
}

#[test]
fn test_execution_mode_slurm_parsing() {
    let yaml = r#"
        mode: slurm
    "#;
    let config: ExecutionConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.mode, ExecutionMode::Slurm);
}

#[test]
fn test_execution_mode_auto_parsing() {
    let yaml = r#"
        mode: auto
    "#;
    let config: ExecutionConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.mode, ExecutionMode::Auto);
}

#[test]
fn test_execution_mode_default_is_auto() {
    let yaml = r#"
        limit_resources: true
    "#;
    let config: ExecutionConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.mode, ExecutionMode::Auto);
}

// =============================================================================
// ExecutionConfig full parsing tests
// =============================================================================

#[test]
fn test_execution_config_direct_mode_full() {
    let yaml = r#"
        mode: direct
        limit_resources: true
        termination_signal: SIGINT
        sigterm_lead_seconds: 45
        sigkill_headroom_seconds: 90
        timeout_exit_code: 124
        oom_exit_code: 139
    "#;
    let config: ExecutionConfig = serde_yaml::from_str(yaml).unwrap();

    assert_eq!(config.mode, ExecutionMode::Direct);
    assert_eq!(config.limit_resources, Some(true));
    assert_eq!(config.termination_signal, Some("SIGINT".to_string()));
    assert_eq!(config.sigterm_lead_seconds, Some(45));
    assert_eq!(config.sigkill_headroom_seconds, Some(90));
    assert_eq!(config.timeout_exit_code, Some(124));
    assert_eq!(config.oom_exit_code, Some(139));
}

#[test]
fn test_execution_config_slurm_mode_full() {
    let yaml = r#"
        mode: slurm
        limit_resources: true
        srun_termination_signal: "TERM@120"
        sigkill_headroom_seconds: 180
        enable_cpu_bind: true
    "#;
    let config: ExecutionConfig = serde_yaml::from_str(yaml).unwrap();

    assert_eq!(config.mode, ExecutionMode::Slurm);
    assert_eq!(config.limit_resources, Some(true));
    assert_eq!(config.srun_termination_signal, Some("TERM@120".to_string()));
    assert_eq!(config.sigkill_headroom_seconds, Some(180));
    assert_eq!(config.enable_cpu_bind, Some(true));
}

#[test]
fn test_execution_config_json_parsing() {
    let json = r#"{
        "mode": "direct",
        "limit_resources": false,
        "termination_signal": "SIGUSR1",
        "sigterm_lead_seconds": 60,
        "sigkill_headroom_seconds": 120
    }"#;
    let config: ExecutionConfig = serde_json::from_str(json).unwrap();

    assert_eq!(config.mode, ExecutionMode::Direct);
    assert_eq!(config.limit_resources, Some(false));
    assert_eq!(config.termination_signal, Some("SIGUSR1".to_string()));
    assert_eq!(config.sigterm_lead_seconds, Some(60));
    assert_eq!(config.sigkill_headroom_seconds, Some(120));
}

// =============================================================================
// Default value tests
// =============================================================================

#[test]
fn test_execution_config_default_values() {
    let config = ExecutionConfig::default();

    assert_eq!(config.mode, ExecutionMode::Auto);
    assert_eq!(config.limit_resources, None);
    assert_eq!(config.termination_signal, None);
    assert_eq!(config.sigterm_lead_seconds, None);
    assert_eq!(config.sigkill_headroom_seconds, None);
    assert_eq!(config.timeout_exit_code, None);
    assert_eq!(config.oom_exit_code, None);
    assert_eq!(config.srun_termination_signal, None);
    assert_eq!(config.enable_cpu_bind, None);
}

#[test]
fn test_execution_config_default_accessor_values() {
    let config = ExecutionConfig::default();

    // Test that accessor methods return expected defaults
    assert!(config.limit_resources()); // default true
    assert_eq!(config.termination_signal(), "SIGTERM");
    assert_eq!(
        config.sigterm_lead_seconds(),
        ExecutionConfig::DEFAULT_SIGTERM_LEAD_SECONDS
    );
    assert_eq!(
        config.sigkill_headroom_seconds(),
        ExecutionConfig::DEFAULT_SIGKILL_HEADROOM_SECONDS
    );
    assert_eq!(
        config.timeout_exit_code(),
        ExecutionConfig::DEFAULT_TIMEOUT_EXIT_CODE
    );
    assert_eq!(
        config.oom_exit_code(),
        ExecutionConfig::DEFAULT_OOM_EXIT_CODE
    );
    assert!(!config.enable_cpu_bind()); // default false
}

#[test]
fn test_execution_config_default_constants() {
    // Verify the constant values match expected Slurm/system conventions
    assert_eq!(ExecutionConfig::DEFAULT_SIGTERM_LEAD_SECONDS, 30);
    assert_eq!(ExecutionConfig::DEFAULT_SIGKILL_HEADROOM_SECONDS, 60);
    assert_eq!(ExecutionConfig::DEFAULT_TIMEOUT_EXIT_CODE, 152); // Slurm TIMEOUT
    assert_eq!(ExecutionConfig::DEFAULT_OOM_EXIT_CODE, 137); // 128 + SIGKILL(9)
}

// =============================================================================
// Effective mode detection tests
// =============================================================================

#[test]
fn test_effective_mode_direct() {
    let config = ExecutionConfig {
        mode: ExecutionMode::Direct,
        ..Default::default()
    };
    assert_eq!(config.effective_mode(), ExecutionMode::Direct);
    assert!(!config.use_srun());
}

#[test]
fn test_effective_mode_slurm() {
    let config = ExecutionConfig {
        mode: ExecutionMode::Slurm,
        ..Default::default()
    };
    assert_eq!(config.effective_mode(), ExecutionMode::Slurm);
    assert!(config.use_srun());
}

#[test]
#[serial]
fn test_effective_mode_auto_without_slurm_env() {
    // Save original value
    let original = std::env::var("SLURM_JOB_ID").ok();
    // Ensure SLURM_JOB_ID is not set
    // SAFETY: Using serial_test to prevent concurrent access to env vars
    unsafe {
        std::env::remove_var("SLURM_JOB_ID");
    }

    let config = ExecutionConfig {
        mode: ExecutionMode::Auto,
        ..Default::default()
    };
    assert_eq!(config.effective_mode(), ExecutionMode::Direct);
    assert!(!config.use_srun());

    // Restore original value
    if let Some(val) = original {
        unsafe {
            std::env::set_var("SLURM_JOB_ID", val);
        }
    }
}

#[test]
#[serial]
fn test_effective_mode_auto_with_slurm_env() {
    // Save original value
    let original = std::env::var("SLURM_JOB_ID").ok();
    // Set SLURM_JOB_ID temporarily
    // SAFETY: Using serial_test to prevent concurrent access to env vars
    unsafe {
        std::env::set_var("SLURM_JOB_ID", "12345");
    }

    let config = ExecutionConfig {
        mode: ExecutionMode::Auto,
        ..Default::default()
    };
    assert_eq!(config.effective_mode(), ExecutionMode::Slurm);
    assert!(config.use_srun());

    // Restore original value
    unsafe {
        if let Some(val) = original {
            std::env::set_var("SLURM_JOB_ID", val);
        } else {
            std::env::remove_var("SLURM_JOB_ID");
        }
    }
}

// =============================================================================
// Validation tests
// =============================================================================

#[test]
fn test_validation_signal_time_less_than_headroom() {
    // Valid: signal at 60s, headroom at 120s - signal will be sent before kill
    let config = ExecutionConfig {
        srun_termination_signal: Some("TERM@60".to_string()),
        sigkill_headroom_seconds: Some(120),
        ..Default::default()
    };
    let warnings = config.validate();
    assert!(
        warnings.is_empty(),
        "Expected no warnings, got: {:?}",
        warnings
    );
}

#[test]
fn test_validation_signal_time_equals_headroom() {
    // Invalid: signal at 60s, headroom at 60s - signal won't be sent in time
    let config = ExecutionConfig {
        srun_termination_signal: Some("TERM@60".to_string()),
        sigkill_headroom_seconds: Some(60),
        ..Default::default()
    };
    let warnings = config.validate();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("TERM@60"));
    assert!(warnings[0].contains("60s"));
    assert!(warnings[0].contains("sigkill_headroom_seconds"));
}

#[test]
fn test_validation_signal_time_exceeds_headroom() {
    // Invalid: signal at 120s, headroom at 60s - signal would be sent after kill
    let config = ExecutionConfig {
        srun_termination_signal: Some("TERM@120".to_string()),
        sigkill_headroom_seconds: Some(60),
        ..Default::default()
    };
    let warnings = config.validate();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("TERM@120"));
    assert!(warnings[0].contains("120s"));
}

#[test]
fn test_validation_signal_without_time() {
    // No time specified in signal - no validation needed
    let config = ExecutionConfig {
        srun_termination_signal: Some("TERM".to_string()),
        sigkill_headroom_seconds: Some(60),
        ..Default::default()
    };
    let warnings = config.validate();
    assert!(warnings.is_empty());
}

#[test]
fn test_validation_no_signal_configured() {
    // No srun_termination_signal - no validation needed
    let config = ExecutionConfig {
        sigkill_headroom_seconds: Some(60),
        ..Default::default()
    };
    let warnings = config.validate();
    assert!(warnings.is_empty());
}

#[test]
fn test_validation_signal_with_default_headroom() {
    // Signal at 90s, default headroom is 60s - should warn
    let config = ExecutionConfig {
        srun_termination_signal: Some("TERM@90".to_string()),
        ..Default::default()
    };
    let warnings = config.validate();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("TERM@90"));
}

// =============================================================================
// WorkflowSpec integration tests
// =============================================================================

#[test]
fn test_workflow_spec_with_execution_config_yaml() {
    let yaml = r#"
        name: test_workflow
        user: test_user
        jobs:
          - name: job1
            command: "echo hello"
        execution_config:
            mode: direct
            termination_signal: SIGTERM
            sigterm_lead_seconds: 30
            sigkill_headroom_seconds: 60
    "#;
    let spec: WorkflowSpec = serde_yaml::from_str(yaml).unwrap();

    assert!(spec.execution_config.is_some());
    let exec_config = spec.execution_config.unwrap();
    assert_eq!(exec_config.mode, ExecutionMode::Direct);
    assert_eq!(exec_config.termination_signal, Some("SIGTERM".to_string()));
    assert_eq!(exec_config.sigterm_lead_seconds, Some(30));
    assert_eq!(exec_config.sigkill_headroom_seconds, Some(60));
}

#[test]
fn test_workflow_spec_with_slurm_execution_config() {
    let yaml = r#"
        name: slurm_workflow
        user: test_user
        jobs:
          - name: job1
            command: "srun hostname"
        execution_config:
            mode: slurm
            limit_resources: true
            srun_termination_signal: "TERM@120"
            sigkill_headroom_seconds: 180
            enable_cpu_bind: false
    "#;
    let spec: WorkflowSpec = serde_yaml::from_str(yaml).unwrap();

    let exec_config = spec.execution_config.unwrap();
    assert_eq!(exec_config.mode, ExecutionMode::Slurm);
    assert_eq!(exec_config.limit_resources, Some(true));
    assert_eq!(
        exec_config.srun_termination_signal,
        Some("TERM@120".to_string())
    );
    assert_eq!(exec_config.sigkill_headroom_seconds, Some(180));
    assert_eq!(exec_config.enable_cpu_bind, Some(false));
}

#[test]
fn test_workflow_spec_without_execution_config() {
    let yaml = r#"
        name: simple_workflow
        user: test_user
        jobs:
          - name: job1
            command: "echo hello"
    "#;
    let spec: WorkflowSpec = serde_yaml::from_str(yaml).unwrap();

    assert!(spec.execution_config.is_none());
}

// =============================================================================
// Serialization round-trip tests
// =============================================================================

#[test]
fn test_execution_config_yaml_roundtrip() {
    let original = ExecutionConfig {
        mode: ExecutionMode::Direct,
        limit_resources: Some(true),
        termination_signal: Some("SIGTERM".to_string()),
        sigterm_lead_seconds: Some(30),
        sigkill_headroom_seconds: Some(60),
        timeout_exit_code: Some(152),
        oom_exit_code: Some(137),
        srun_termination_signal: None,
        enable_cpu_bind: None,
        staggered_start: None,
        stdio: None,
        job_stdio_overrides: None,
    };

    let yaml = serde_yaml::to_string(&original).unwrap();
    let parsed: ExecutionConfig = serde_yaml::from_str(&yaml).unwrap();

    assert_eq!(original, parsed);
}

#[test]
fn test_execution_config_json_roundtrip() {
    let original = ExecutionConfig {
        mode: ExecutionMode::Slurm,
        limit_resources: Some(true),
        termination_signal: None,
        sigterm_lead_seconds: None,
        sigkill_headroom_seconds: Some(120),
        timeout_exit_code: None,
        oom_exit_code: None,
        srun_termination_signal: Some("TERM@90".to_string()),
        enable_cpu_bind: Some(true),
        staggered_start: None,
        stdio: None,
        job_stdio_overrides: None,
    };

    let json = serde_json::to_string(&original).unwrap();
    let parsed: ExecutionConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(original, parsed);
}

// =============================================================================
// StdioConfig unit tests
// =============================================================================

#[test]
fn test_stdio_for_job_returns_default_when_no_config() {
    let config = ExecutionConfig::default();
    let stdio = config.stdio_for_job("any_job");
    assert_eq!(stdio.mode, StdioMode::Separate);
    assert_eq!(stdio.delete_on_success, None);
}

#[test]
fn test_stdio_for_job_returns_workflow_level_config() {
    let config = ExecutionConfig {
        stdio: Some(StdioConfig {
            mode: StdioMode::Combined,
            delete_on_success: Some(true),
        }),
        ..Default::default()
    };
    let stdio = config.stdio_for_job("any_job");
    assert_eq!(stdio.mode, StdioMode::Combined);
    assert_eq!(stdio.delete_on_success, Some(true));
}

#[test]
fn test_stdio_for_job_per_job_override_takes_precedence() {
    let mut overrides = HashMap::new();
    overrides.insert(
        "special_job".to_string(),
        StdioConfig {
            mode: StdioMode::None,
            delete_on_success: Some(false),
        },
    );
    let config = ExecutionConfig {
        stdio: Some(StdioConfig {
            mode: StdioMode::Combined,
            delete_on_success: Some(true),
        }),
        job_stdio_overrides: Some(overrides),
        ..Default::default()
    };

    // Overridden job gets its own config
    let special = config.stdio_for_job("special_job");
    assert_eq!(special.mode, StdioMode::None);
    assert_eq!(special.delete_on_success, Some(false));

    // Other jobs fall back to workflow-level
    let normal = config.stdio_for_job("other_job");
    assert_eq!(normal.mode, StdioMode::Combined);
    assert_eq!(normal.delete_on_success, Some(true));
}

#[test]
fn test_delete_stdio_on_success_defaults_to_false() {
    let config = ExecutionConfig::default();
    assert!(!config.delete_stdio_on_success("any_job"));
}

#[test]
fn test_delete_stdio_on_success_respects_workflow_config() {
    let config = ExecutionConfig {
        stdio: Some(StdioConfig {
            mode: StdioMode::Separate,
            delete_on_success: Some(true),
        }),
        ..Default::default()
    };
    assert!(config.delete_stdio_on_success("any_job"));
}

#[test]
fn test_delete_stdio_on_success_respects_per_job_override() {
    let mut overrides = HashMap::new();
    overrides.insert(
        "keep_logs".to_string(),
        StdioConfig {
            mode: StdioMode::Separate,
            delete_on_success: Some(false),
        },
    );
    let config = ExecutionConfig {
        stdio: Some(StdioConfig {
            mode: StdioMode::Separate,
            delete_on_success: Some(true),
        }),
        job_stdio_overrides: Some(overrides),
        ..Default::default()
    };
    assert!(!config.delete_stdio_on_success("keep_logs"));
    assert!(config.delete_stdio_on_success("other_job"));
}

#[test]
fn test_stdio_config_yaml_parsing() {
    let yaml = r#"
        name: stdio_test
        user: test_user
        jobs:
          - name: job1
            command: "echo hello"
        execution_config:
            stdio:
                mode: combined
                delete_on_success: true
    "#;
    let spec: WorkflowSpec = serde_yaml::from_str(yaml).unwrap();
    let exec = spec.execution_config.unwrap();
    let stdio = exec.stdio.unwrap();
    assert_eq!(stdio.mode, StdioMode::Combined);
    assert_eq!(stdio.delete_on_success, Some(true));
}

#[test]
fn test_stdio_mode_all_variants_yaml_parsing() {
    for (yaml_val, expected) in [
        ("separate", StdioMode::Separate),
        ("combined", StdioMode::Combined),
        ("no_stdout", StdioMode::NoStdout),
        ("no_stderr", StdioMode::NoStderr),
        ("none", StdioMode::None),
    ] {
        let yaml = format!(
            r#"
            name: stdio_test
            user: test_user
            jobs:
              - name: job1
                command: "echo hello"
            execution_config:
                stdio:
                    mode: {}
        "#,
            yaml_val
        );
        let spec: WorkflowSpec = serde_yaml::from_str(&yaml).unwrap();
        let exec = spec.execution_config.unwrap();
        assert_eq!(
            exec.stdio.unwrap().mode,
            expected,
            "Failed for YAML value '{}'",
            yaml_val
        );
    }
}

#[test]
fn test_per_job_stdio_override_yaml_parsing() {
    let yaml = r#"
        name: per_job_stdio_test
        user: test_user
        jobs:
          - name: job1
            command: "echo hello"
            stdio:
                mode: none
          - name: job2
            command: "echo world"
            stdio:
                mode: combined
                delete_on_success: true
          - name: job3
            command: "echo default"
    "#;
    let spec: WorkflowSpec = serde_yaml::from_str(yaml).unwrap();

    // job1 has per-job override
    assert_eq!(spec.jobs[0].stdio.as_ref().unwrap().mode, StdioMode::None);

    // job2 has per-job override with delete_on_success
    let job2_stdio = spec.jobs[1].stdio.as_ref().unwrap();
    assert_eq!(job2_stdio.mode, StdioMode::Combined);
    assert_eq!(job2_stdio.delete_on_success, Some(true));

    // job3 has no override
    assert!(spec.jobs[2].stdio.is_none());
}

#[test]
fn test_stdio_config_roundtrip() {
    let original = ExecutionConfig {
        stdio: Some(StdioConfig {
            mode: StdioMode::NoStderr,
            delete_on_success: Some(true),
        }),
        job_stdio_overrides: Some(HashMap::from([(
            "special".to_string(),
            StdioConfig {
                mode: StdioMode::None,
                delete_on_success: Some(false),
            },
        )])),
        ..Default::default()
    };

    let yaml = serde_yaml::to_string(&original).unwrap();
    let parsed: ExecutionConfig = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(original, parsed);

    let json = serde_json::to_string(&original).unwrap();
    let parsed: ExecutionConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(original, parsed);
}

// =============================================================================
// KDL stdio parsing tests
// =============================================================================

#[test]
fn test_stdio_config_kdl_parsing() {
    let kdl = r#"
        name "stdio_kdl_test"
        user "test_user"
        job "job1" {
            command "echo hello"
        }
        execution_config {
            mode "direct"
            stdio {
                mode "combined"
                delete_on_success #true
            }
        }
    "#;
    let spec = WorkflowSpec::from_spec_file_content(kdl, "kdl").expect("Failed to parse KDL");
    let exec = spec.execution_config.unwrap();
    let stdio = exec.stdio.unwrap();
    assert_eq!(stdio.mode, StdioMode::Combined);
    assert_eq!(stdio.delete_on_success, Some(true));
}

#[test]
fn test_stdio_mode_all_variants_kdl_parsing() {
    for (kdl_val, expected) in [
        ("separate", StdioMode::Separate),
        ("combined", StdioMode::Combined),
        ("no_stdout", StdioMode::NoStdout),
        ("no_stderr", StdioMode::NoStderr),
        ("none", StdioMode::None),
    ] {
        let kdl = format!(
            r#"
            name "stdio_kdl_test"
            user "test_user"
            job "job1" {{
                command "echo hello"
            }}
            execution_config {{
                mode "direct"
                stdio {{
                    mode "{}"
                }}
            }}
        "#,
            kdl_val
        );
        let spec = WorkflowSpec::from_spec_file_content(&kdl, "kdl").expect("Failed to parse KDL");
        let exec = spec.execution_config.unwrap();
        assert_eq!(
            exec.stdio.unwrap().mode,
            expected,
            "Failed for KDL value '{}'",
            kdl_val
        );
    }
}

#[test]
fn test_per_job_stdio_override_kdl_parsing() {
    let kdl = r#"
        name "per_job_stdio_kdl_test"
        user "test_user"
        job "job1" {
            command "echo hello"
            stdio {
                mode "none"
            }
        }
        job "job2" {
            command "echo world"
            stdio {
                mode "combined"
                delete_on_success #true
            }
        }
        job "job3" {
            command "echo default"
        }
    "#;
    let spec = WorkflowSpec::from_spec_file_content(kdl, "kdl").expect("Failed to parse KDL");

    // job1 has per-job override
    assert_eq!(spec.jobs[0].stdio.as_ref().unwrap().mode, StdioMode::None);

    // job2 has per-job override with delete_on_success
    let job2_stdio = spec.jobs[1].stdio.as_ref().unwrap();
    assert_eq!(job2_stdio.mode, StdioMode::Combined);
    assert_eq!(job2_stdio.delete_on_success, Some(true));

    // job3 has no override
    assert!(spec.jobs[2].stdio.is_none());
}

#[test]
fn test_stdio_config_kdl_roundtrip() {
    let yaml = r#"
        name: stdio_roundtrip_test
        user: test_user
        jobs:
          - name: job1
            command: "echo hello"
            stdio:
                mode: none
          - name: job2
            command: "echo world"
        execution_config:
            mode: direct
            stdio:
                mode: no_stderr
                delete_on_success: true
    "#;
    let spec: WorkflowSpec = serde_yaml::from_str(yaml).unwrap();

    // Serialize to KDL and parse back
    let kdl_str = spec.to_kdl_str();
    let roundtripped =
        WorkflowSpec::from_spec_file_content(&kdl_str, "kdl").expect("Failed to parse KDL");

    // Verify execution_config stdio survived the roundtrip
    let exec = roundtripped.execution_config.unwrap();
    let stdio = exec.stdio.unwrap();
    assert_eq!(stdio.mode, StdioMode::NoStderr);
    assert_eq!(stdio.delete_on_success, Some(true));

    // Verify per-job stdio survived the roundtrip
    assert_eq!(
        roundtripped.jobs[0].stdio.as_ref().unwrap().mode,
        StdioMode::None
    );
    assert!(roundtripped.jobs[1].stdio.is_none());
}

// =============================================================================
// Server integration tests
// =============================================================================

#[rstest]
fn test_create_workflow_with_execution_config(start_server: &ServerProcess) {
    let yaml = r#"
        name: execution_config_test
        user: test_user
        jobs:
          - name: job1
            command: "echo hello"
        execution_config:
            mode: direct
            termination_signal: SIGTERM
            sigterm_lead_seconds: 45
            sigkill_headroom_seconds: 90
    "#;

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), yaml).expect("Failed to write workflow file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false,
    )
    .expect("Failed to create workflow from spec file");

    assert!(workflow_id > 0);

    // Verify the execution_config was stored
    let workflow = default_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");
    assert_eq!(workflow.name, "execution_config_test");

    assert!(workflow.execution_config.is_some());
    let exec_config: ExecutionConfig =
        serde_json::from_str(workflow.execution_config.as_ref().unwrap()).unwrap();
    assert_eq!(exec_config.mode, ExecutionMode::Direct);
    assert_eq!(exec_config.sigterm_lead_seconds, Some(45));
    assert_eq!(exec_config.sigkill_headroom_seconds, Some(90));
}

#[rstest]
fn test_create_workflow_with_slurm_execution_config(start_server: &ServerProcess) {
    let yaml = r#"
        name: slurm_execution_config_test
        user: test_user
        jobs:
          - name: job1
            command: "hostname"
        execution_config:
            mode: slurm
            srun_termination_signal: "TERM@120"
            sigkill_headroom_seconds: 180
            enable_cpu_bind: true
    "#;

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), yaml).expect("Failed to write workflow file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false,
    )
    .expect("Failed to create workflow from spec file");

    // Verify the execution_config was stored correctly
    let workflow = default_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");

    assert!(workflow.execution_config.is_some());
    let exec_config: ExecutionConfig =
        serde_json::from_str(workflow.execution_config.as_ref().unwrap()).unwrap();
    assert_eq!(exec_config.mode, ExecutionMode::Slurm);
    assert_eq!(
        exec_config.srun_termination_signal,
        Some("TERM@120".to_string())
    );
    assert_eq!(exec_config.sigkill_headroom_seconds, Some(180));
    assert_eq!(exec_config.enable_cpu_bind, Some(true));
}

#[rstest]
fn test_create_workflow_without_execution_config(start_server: &ServerProcess) {
    let yaml = r#"
        name: no_execution_config_test
        user: test_user
        jobs:
          - name: job1
            command: "echo hello"
    "#;

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), yaml).expect("Failed to write workflow file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false,
    )
    .expect("Failed to create workflow from spec file");

    // Workflow should be created successfully without execution_config
    let workflow = default_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");
    assert_eq!(workflow.name, "no_execution_config_test");
    // execution_config may be None or empty
}

#[rstest]
fn test_create_workflow_with_auto_mode(start_server: &ServerProcess) {
    let yaml = r#"
        name: auto_mode_test
        user: test_user
        jobs:
          - name: job1
            command: "echo hello"
        execution_config:
            mode: auto
            sigkill_headroom_seconds: 120
    "#;

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), yaml).expect("Failed to write workflow file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false,
    )
    .expect("Failed to create workflow from spec file");

    let workflow = default_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");

    assert!(workflow.execution_config.is_some());
    let exec_config: ExecutionConfig =
        serde_json::from_str(workflow.execution_config.as_ref().unwrap()).unwrap();
    assert_eq!(exec_config.mode, ExecutionMode::Auto);
    assert_eq!(exec_config.sigkill_headroom_seconds, Some(120));
}

// =============================================================================
// KDL format tests
// =============================================================================

#[test]
fn test_execution_config_kdl_parsing() {
    let kdl = r#"
        name "kdl_workflow"
        user "test_user"

        execution_config {
            mode "direct"
            termination_signal "SIGTERM"
            sigterm_lead_seconds 30
            sigkill_headroom_seconds 90
            timeout_exit_code 152
        }

        job "job1" {
            command "echo hello"
        }
    "#;
    let spec: WorkflowSpec =
        WorkflowSpec::from_spec_file_content(kdl, "kdl").expect("Failed to parse KDL");

    assert!(spec.execution_config.is_some());
    let exec_config = spec.execution_config.unwrap();
    assert_eq!(exec_config.mode, ExecutionMode::Direct);
    assert_eq!(exec_config.termination_signal, Some("SIGTERM".to_string()));
    assert_eq!(exec_config.sigterm_lead_seconds, Some(30));
    assert_eq!(exec_config.sigkill_headroom_seconds, Some(90));
    assert_eq!(exec_config.timeout_exit_code, Some(152));
}

#[test]
fn test_execution_config_kdl_slurm_mode() {
    let kdl = r#"
        name "kdl_slurm_workflow"
        user "test_user"

        execution_config {
            mode "slurm"
            srun_termination_signal "TERM@120"
            sigkill_headroom_seconds 180
            enable_cpu_bind #true
        }

        job "job1" {
            command "hostname"
        }
    "#;
    let spec: WorkflowSpec =
        WorkflowSpec::from_spec_file_content(kdl, "kdl").expect("Failed to parse KDL");

    let exec_config = spec.execution_config.unwrap();
    assert_eq!(exec_config.mode, ExecutionMode::Slurm);
    assert_eq!(
        exec_config.srun_termination_signal,
        Some("TERM@120".to_string())
    );
    assert_eq!(exec_config.sigkill_headroom_seconds, Some(180));
    assert_eq!(exec_config.enable_cpu_bind, Some(true));
}

#[test]
fn test_execution_config_kdl_roundtrip() {
    // Create a workflow with execution_config via YAML (simpler than struct initialization)
    let yaml = r#"
        name: roundtrip_test
        user: test_user
        jobs:
          - name: job1
            command: "echo hello"
        execution_config:
            mode: direct
            limit_resources: true
            termination_signal: SIGTERM
            sigterm_lead_seconds: 45
            sigkill_headroom_seconds: 90
            timeout_exit_code: 152
            oom_exit_code: 137
    "#;
    let original: WorkflowSpec = serde_yaml::from_str(yaml).expect("Failed to parse YAML");

    // Convert to KDL and back
    let kdl_str = original.to_kdl_str();
    let parsed =
        WorkflowSpec::from_spec_file_content(&kdl_str, "kdl").expect("Failed to parse KDL");

    assert!(parsed.execution_config.is_some());
    let exec_config = parsed.execution_config.unwrap();
    assert_eq!(exec_config.mode, ExecutionMode::Direct);
    assert_eq!(exec_config.limit_resources, Some(true));
    assert_eq!(exec_config.termination_signal, Some("SIGTERM".to_string()));
    assert_eq!(exec_config.sigterm_lead_seconds, Some(45));
    assert_eq!(exec_config.sigkill_headroom_seconds, Some(90));
}

// =============================================================================
// Edge case tests
// =============================================================================

#[test]
fn test_execution_config_with_all_fields() {
    let yaml = r#"
        mode: direct
        limit_resources: true
        termination_signal: SIGINT
        sigterm_lead_seconds: 15
        sigkill_headroom_seconds: 45
        timeout_exit_code: 200
        oom_exit_code: 201
        srun_termination_signal: "USR1@30"
        enable_cpu_bind: true
    "#;
    let config: ExecutionConfig = serde_yaml::from_str(yaml).unwrap();

    // All fields should be set even though some don't make sense for direct mode
    assert_eq!(config.mode, ExecutionMode::Direct);
    assert_eq!(config.limit_resources, Some(true));
    assert_eq!(config.termination_signal, Some("SIGINT".to_string()));
    assert_eq!(config.sigterm_lead_seconds, Some(15));
    assert_eq!(config.sigkill_headroom_seconds, Some(45));
    assert_eq!(config.timeout_exit_code, Some(200));
    assert_eq!(config.oom_exit_code, Some(201));
    assert_eq!(config.srun_termination_signal, Some("USR1@30".to_string()));
    assert_eq!(config.enable_cpu_bind, Some(true));
}

#[test]
fn test_execution_config_limit_resources_false() {
    let config = ExecutionConfig {
        limit_resources: Some(false),
        ..Default::default()
    };
    assert!(!config.limit_resources());
}

#[test]
fn test_execution_config_custom_exit_codes() {
    let config = ExecutionConfig {
        timeout_exit_code: Some(124),
        oom_exit_code: Some(125),
        ..Default::default()
    };
    assert_eq!(config.timeout_exit_code(), 124);
    assert_eq!(config.oom_exit_code(), 125);
}

#[test]
fn test_validation_with_various_signal_formats() {
    // Test various srun signal formats
    let test_cases = vec![
        ("TERM@30", Some(30)),
        ("INT@60", Some(60)),
        ("USR1@120", Some(120)),
        ("TERM", None),     // No time
        ("KILL", None),     // No time
        ("@60", Some(60)),  // Just time (unusual but parseable)
        ("TERM@abc", None), // Invalid time
        ("TERM@", None),    // Empty time
    ];

    for (signal_spec, expected_seconds) in test_cases {
        let config = ExecutionConfig {
            srun_termination_signal: Some(signal_spec.to_string()),
            sigkill_headroom_seconds: Some(90),
            ..Default::default()
        };
        let warnings = config.validate();

        match expected_seconds {
            Some(secs) if secs >= 90 => {
                assert!(
                    !warnings.is_empty(),
                    "Expected warning for {} >= 90s headroom",
                    signal_spec
                );
            }
            Some(secs) if secs < 90 => {
                assert!(
                    warnings.is_empty(),
                    "Expected no warning for {} < 90s headroom, got: {:?}",
                    signal_spec,
                    warnings
                );
            }
            None => {
                assert!(
                    warnings.is_empty(),
                    "Expected no warning for {} without time, got: {:?}",
                    signal_spec,
                    warnings
                );
            }
            _ => {}
        }
    }
}

// =============================================================================
// Direct mode job execution integration tests
// (merged from test_direct_mode_execution.rs)
// =============================================================================

#[rstest]
fn test_direct_mode_simple_job_execution(start_server: &ServerProcess) {
    // Create a simple workflow with direct mode execution
    let yaml = r#"
        name: direct_mode_simple_test
        user: test_user
        jobs:
          - name: simple_job
            command: "echo 'Direct mode test'"
        execution_config:
            mode: direct
    "#;

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), yaml).expect("Failed to write workflow file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false,
    )
    .expect("Failed to create workflow");

    // Verify workflow was created with direct mode execution_config
    let workflow = default_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");

    assert!(workflow.execution_config.is_some());
    let exec_config: ExecutionConfig =
        serde_json::from_str(workflow.execution_config.as_ref().unwrap()).unwrap();
    assert_eq!(exec_config.mode, ExecutionMode::Direct);
}

#[rstest]
fn test_direct_mode_with_resource_limits(start_server: &ServerProcess) {
    // Create workflow with direct mode and resource limits enabled
    let yaml = r#"
        name: direct_mode_resource_limits
        user: test_user
        resource_requirements:
          - name: small
            num_cpus: 1
            memory: 256m
            runtime: PT1M
        jobs:
          - name: limited_job
            command: "echo 'Running with resource limits'"
            resource_requirements: small
        execution_config:
            mode: direct
            limit_resources: true
    "#;

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), yaml).expect("Failed to write workflow file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false,
    )
    .expect("Failed to create workflow");

    let workflow = default_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");

    let exec_config: ExecutionConfig =
        serde_json::from_str(workflow.execution_config.as_ref().unwrap()).unwrap();

    assert_eq!(exec_config.mode, ExecutionMode::Direct);
    assert_eq!(exec_config.limit_resources, Some(true));
}

#[rstest]
fn test_direct_mode_disabled_resource_limits(start_server: &ServerProcess) {
    // Test that limit_resources: false is respected
    let yaml = r#"
        name: direct_mode_no_limits
        user: test_user
        jobs:
          - name: unlimited_job
            command: "echo 'No resource limits'"
        execution_config:
            mode: direct
            limit_resources: false
    "#;

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), yaml).expect("Failed to write workflow file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false,
    )
    .expect("Failed to create workflow");

    let workflow = default_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");

    let exec_config: ExecutionConfig =
        serde_json::from_str(workflow.execution_config.as_ref().unwrap()).unwrap();

    assert!(!exec_config.limit_resources());
}

#[rstest]
fn test_direct_mode_custom_exit_codes(start_server: &ServerProcess) {
    // Test custom timeout and OOM exit codes
    let yaml = r#"
        name: custom_exit_codes_test
        user: test_user
        jobs:
          - name: job1
            command: "echo test"
        execution_config:
            mode: direct
            timeout_exit_code: 200
            oom_exit_code: 201
    "#;

    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    fs::write(temp_file.path(), yaml).expect("Failed to write workflow file");

    let workflow_id = WorkflowSpec::create_workflow_from_spec(
        &start_server.config,
        temp_file.path(),
        "test_user",
        false,
        false,
    )
    .expect("Failed to create workflow");

    let workflow = default_api::get_workflow(&start_server.config, workflow_id)
        .expect("Failed to get workflow");

    let exec_config: ExecutionConfig =
        serde_json::from_str(workflow.execution_config.as_ref().unwrap()).unwrap();

    assert_eq!(exec_config.timeout_exit_code(), 200);
    assert_eq!(exec_config.oom_exit_code(), 201);
}

#[test]
fn test_direct_mode_kdl_with_limit_resources() {
    let kdl = r#"
        name "kdl_direct_mode"
        user "test_user"

        execution_config {
            mode "direct"
            limit_resources #true
            termination_signal "SIGTERM"
            sigterm_lead_seconds 30
            sigkill_headroom_seconds 60
        }

        job "job1" {
            command "echo hello"
        }
    "#;

    let spec = WorkflowSpec::from_spec_file_content(kdl, "kdl").expect("Failed to parse KDL");

    let exec_config = spec.execution_config.unwrap();
    assert_eq!(exec_config.mode, ExecutionMode::Direct);
    assert_eq!(exec_config.limit_resources, Some(true));
    assert_eq!(exec_config.termination_signal, Some("SIGTERM".to_string()));
    assert_eq!(exec_config.sigterm_lead_seconds, Some(30));
    assert_eq!(exec_config.sigkill_headroom_seconds, Some(60));
}
