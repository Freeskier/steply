use super::invalid_yaml_message;

#[test]
fn rejects_on_submit_without_direct_value_binding() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: text_input
        id: derived_name
        label: Derived Name
        reads: profile.name
        writes:
          profile.slug: "{{ value }}"
        commit_policy: on_submit
"#;

    let err = invalid_yaml_message(yaml);
    assert!(err.contains("binding.commit_policy=on_submit"));
}

#[test]
fn rejects_overlapping_widget_writes_in_the_same_step() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: text_input
        id: profile_name
        label: Profile Name
        reads: profile.name
        writes:
          profile: "{{ value }}"
      - type: text_input
        id: profile_slug
        label: Profile Slug
        reads: profile.slug
        writes:
          profile.name: "{{ value }}"
"#;

    let err = invalid_yaml_message(yaml);
    assert!(err.contains("overlapping widget writes"));
    assert!(err.contains("profile"));
}

#[test]
fn rejects_cross_owner_widget_overlaps_across_steps() {
    let yaml = r#"
version: 1
steps:
  - id: first
    title: First
    widgets:
      - type: text_input
        id: profile_name
        label: Profile Name
        value: profile.name
        writes:
          profile.name: "{{ value }}"
  - id: second
    title: Second
    widgets:
      - type: text_input
        id: profile_object
        label: Profile Object
        reads: payload
        writes:
          profile: "{{ value }}"
"#;

    let err = invalid_yaml_message(yaml);
    assert!(err.contains("overlapping selectors cannot be owned by different writer kinds"));
}

#[test]
fn rejects_task_writes_overlapping_widget_writes() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: text_input
        id: profile_name
        label: Profile Name
        value: profile.name
        writes:
          profile.name: "{{ value }}"
tasks:
  - id: fill_profile
    kind: exec
    program: cat
    writes:
      profile: "{{ result }}"
"#;

    let err = invalid_yaml_message(yaml);
    assert!(err.contains("task 'fill_profile' writes 'profile'"));
    assert!(err.contains("overlaps with"));
}

#[test]
fn rejects_overlapping_task_writes() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets: []
tasks:
  - id: fill_profile
    kind: exec
    program: cat
    writes:
      profile: "{{ result }}"
  - id: fill_name
    kind: exec
    program: cat
    writes:
      profile.name: "{{ result }}"
"#;

    let err = invalid_yaml_message(yaml);
    assert!(err.contains("overlapping task writes are not allowed"));
}

#[test]
fn rejects_binding_cycles_between_widgets() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: text_input
        id: first
        label: First
        reads: profile.second
        writes:
          profile.first: "{{ value }}"
      - type: text_input
        id: second
        label: Second
        reads: profile.first
        writes:
          profile.second: "{{ value }}"
"#;

    let err = invalid_yaml_message(yaml);
    assert!(err.contains("binding cycle"));
}

#[test]
fn rejects_writes_on_read_only_output_bindings() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: progress_output
        id: progress
        label: Progress
        reads: demo.progress
        writes:
          demo.progress_echo: "{{ value }}"
"#;

    let err = invalid_yaml_message(yaml);
    assert!(err.contains("progress_output"));
    assert!(err.contains("binding.reads only"));
}

#[test]
fn rejects_value_binding_on_writes_only_widgets() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: snippet
        id: deploy_cmd
        label: Deploy command
        template: "deploy <service>"
        value: demo.command
        inputs:
          - type: text_input
            id: service
            label: Service
"#;

    let err = invalid_yaml_message(yaml);
    assert!(err.contains("snippet"));
    assert!(err.contains("binding.writes only"));
}

#[test]
fn rejects_reads_binding_on_command_runner_root() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: command_runner
        id: fetch
        label: Fetch
        reads: demo.name
        commands:
          - label: Echo
            program: cat
"#;

    let err = invalid_yaml_message(yaml);
    assert!(err.contains("command_runner"));
    assert!(err.contains("binding.writes only"));
}

#[test]
fn rejects_repeater_iterate_object_literal() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: repeater
        id: rows
        label: Rows
        iterate:
          invalid: shape
        widgets:
          - type: text_input
            id: name
            label: Name
"#;

    let err = invalid_yaml_message(yaml);
    assert!(err.contains("repeater iterate"));
}

#[test]
fn allows_widget_when_referring_to_task_written_root() {
    let yaml = r#"
version: 1
tasks:
  - id: remaining_files
    kind: exec
    program: cat
    writes: demo.remaining_files
steps:
  - id: demo
    title: Demo
    widgets:
      - type: text_output
        id: info
        text: "Remaining: {{demo.remaining_files}}"
        when:
          ref: demo.remaining_files
"#;

    crate::config::load_from_yaml_str(yaml).expect("yaml should validate");
}

#[test]
fn allows_repeater_child_conditions_to_use_private_scope_roots() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    widgets:
      - type: repeater
        id: rows
        label: Rows
        iterate: 2
        widgets:
          - type: text_output
            id: intro
            text: "First row only"
            when:
              ref: _index
              is: equals
              value: 0
"#;

    crate::config::load_from_yaml_str(yaml).expect("yaml should validate");
}

#[test]
fn rejects_condition_without_mode() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    when: {}
    widgets: []
"#;

    let err = invalid_yaml_message(yaml);
    assert!(err.contains("condition must contain one of 'ref', 'all', 'any', or 'not'"));
}

#[test]
fn rejects_condition_that_mixes_ref_and_all() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    when:
      ref: demo.enabled
      all:
        - ref: demo.enabled
    widgets: []
"#;

    let err = invalid_yaml_message(yaml);
    assert!(err.contains("condition cannot mix 'ref' with 'all', 'any', or 'not'"));
}

#[test]
fn rejects_condition_operator_without_value() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    when:
      ref: demo.count
      is: greater_than
    widgets:
      - type: slider
        id: count
        label: Count
        min: 0
        max: 10
        default: 0
        value: demo.count
"#;

    let err = invalid_yaml_message(yaml);
    assert!(err.contains("condition operator 'greater_than' requires 'value'"));
}

#[test]
fn rejects_value_on_truthy_condition() {
    let yaml = r#"
version: 1
steps:
  - id: demo
    title: Demo
    when:
      ref: demo.enabled
      value: true
    widgets:
      - type: checkbox
        id: enabled
        label: Enabled
        default: true
        value: demo.enabled
"#;

    let err = invalid_yaml_message(yaml);
    assert!(err.contains("condition operator 'truthy' does not allow 'value'"));
}
