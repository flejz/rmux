use super::*;

#[tokio::test]
async fn if_shell_format_mode_dispatches_selected_rmux_command() {
    let handler = RequestHandler::new();

    let response = handler
        .handle(Request::IfShell(IfShellRequest {
            condition: "1".to_owned(),
            format_mode: true,
            then_command: "set-buffer -b chosen selected".to_owned(),
            else_command: Some("set-buffer -b chosen wrong".to_owned()),
            target: None,
            caller_cwd: None,
            background: false,
        }))
        .await;

    assert_eq!(
        response,
        Response::IfShell(rmux_proto::IfShellResponse::no_output())
    );

    let response = handler
        .handle(Request::ShowBuffer(ShowBufferRequest {
            name: Some("chosen".to_owned()),
        }))
        .await;
    assert_eq!(
        response
            .command_output()
            .expect("show-buffer output")
            .stdout(),
        b"selected"
    );
}

#[tokio::test]
async fn if_shell_false_without_else_is_a_successful_noop() {
    let handler = RequestHandler::new();

    let response = handler
        .handle(Request::IfShell(IfShellRequest {
            condition: "0".to_owned(),
            format_mode: true,
            then_command: "set-buffer impossible".to_owned(),
            else_command: None,
            target: None,
            caller_cwd: None,
            background: false,
        }))
        .await;

    assert_eq!(
        response,
        Response::IfShell(rmux_proto::IfShellResponse::no_output())
    );
}

#[tokio::test]
async fn scripted_pane_commands_accept_session_targets_like_tmux() {
    let handler = RequestHandler::new();
    let alpha = session_name("alpha");

    assert!(matches!(
        handler
            .handle(Request::NewSession(NewSessionRequest {
                session_name: alpha.clone(),
                detached: true,
                size: Some(TerminalSize { cols: 80, rows: 24 }),
                environment: None,
            }))
            .await,
        Response::NewSession(_)
    ));

    let response = handler
        .handle(Request::IfShell(IfShellRequest {
            condition: "1".to_owned(),
            format_mode: true,
            then_command: "copy-mode -t alpha".to_owned(),
            else_command: None,
            target: None,
            caller_cwd: None,
            background: false,
        }))
        .await;
    assert!(matches!(response, Response::IfShell(_)));

    let mode = handler
        .handle(Request::DisplayMessage(DisplayMessageRequest {
            target: Some(Target::Pane(PaneTarget::new(alpha, 0))),
            print: true,
            message: Some("#{pane_in_mode}".to_owned()),
        }))
        .await;
    let output = mode.command_output().expect("display-message output");
    assert_eq!(output.stdout(), b"1\n");
}

#[tokio::test]
async fn if_shell_shell_mode_uses_tmux_shell_environment_and_caller_cwd() {
    let handler = RequestHandler::new();
    let alpha = session_name("alpha");
    let root = temp_root("if-shell-shell-mode");
    let marker = root.join("shell-used.txt");
    let shell_path = root.join("record-shell.sh");

    write_executable_script(
        &shell_path,
        &format!(
            "#!/bin/sh\nprintf used > {}\nexec /bin/sh \"$@\"\n",
            shell_quote(&marker)
        ),
    );

    assert!(matches!(
        handler
            .handle(Request::NewSession(NewSessionRequest {
                session_name: alpha.clone(),
                detached: true,
                size: Some(TerminalSize { cols: 80, rows: 24 }),
                environment: None,
            }))
            .await,
        Response::NewSession(_)
    ));
    assert!(matches!(
        handler
            .handle(Request::SetOption(SetOptionRequest {
                scope: ScopeSelector::Global,
                option: OptionName::DefaultShell,
                value: shell_path.to_string_lossy().into_owned(),
                mode: SetOptionMode::Replace,
            }))
            .await,
        Response::SetOption(_)
    ));
    assert!(matches!(
        handler
            .handle(Request::SetEnvironment(SetEnvironmentRequest {
                scope: ScopeSelector::Session(alpha.clone()),
                name: "FOO".to_owned(),
                value: "bar".to_owned(),
                mode: None,
                hidden: false,
                format: false,
            }))
            .await,
        Response::SetEnvironment(_)
    ));

    let response = handler
        .handle(Request::IfShell(IfShellRequest {
            condition: format!(
                "test \"$FOO\" = bar && test \"$PWD\" = {}",
                shell_quote(&root)
            ),
            format_mode: false,
            then_command: "set-buffer -b chosen yes".to_owned(),
            else_command: Some("set-buffer -b chosen no".to_owned()),
            target: Some(Target::Session(alpha)),
            caller_cwd: Some(root),
            background: false,
        }))
        .await;

    assert_eq!(
        response,
        Response::IfShell(rmux_proto::IfShellResponse::no_output())
    );
    assert_eq!(
        handler
            .handle(Request::ShowBuffer(ShowBufferRequest {
                name: Some("chosen".to_owned()),
            }))
            .await
            .command_output()
            .expect("show-buffer output")
            .stdout(),
        b"yes"
    );
    assert_eq!(fs::read_to_string(marker).expect("shell marker"), "used");
}

#[tokio::test]
async fn if_shell_nested_set_buffer_accepts_hyphen_prefixed_content() {
    let handler = RequestHandler::new();

    let response = handler
        .handle(Request::IfShell(IfShellRequest {
            condition: "1".to_owned(),
            format_mode: true,
            then_command: "set-buffer -b hyphen -value".to_owned(),
            else_command: None,
            target: None,
            caller_cwd: None,
            background: false,
        }))
        .await;

    assert_eq!(
        response,
        Response::IfShell(rmux_proto::IfShellResponse::no_output())
    );

    let response = handler
        .handle(Request::ShowBuffer(ShowBufferRequest {
            name: Some("hyphen".to_owned()),
        }))
        .await;
    assert_eq!(
        response
            .command_output()
            .expect("show-buffer output")
            .stdout(),
        b"-value"
    );
}

#[tokio::test]
async fn if_shell_nested_wait_for_accepts_hyphen_prefixed_channel_after_mode_flag() {
    let handler = RequestHandler::new();

    let response = handler
        .handle(Request::IfShell(IfShellRequest {
            condition: "1".to_owned(),
            format_mode: true,
            then_command: "wait-for -S -channel".to_owned(),
            else_command: None,
            target: None,
            caller_cwd: None,
            background: false,
        }))
        .await;

    assert_eq!(
        response,
        Response::IfShell(rmux_proto::IfShellResponse::no_output())
    );
}

#[tokio::test]
async fn if_shell_nested_run_shell_accepts_double_dash_before_command() {
    let handler = RequestHandler::new();

    let response = handler
        .handle(Request::IfShell(IfShellRequest {
            condition: "1".to_owned(),
            format_mode: true,
            then_command: "run-shell -- true".to_owned(),
            else_command: None,
            target: None,
            caller_cwd: None,
            background: false,
        }))
        .await;

    assert_eq!(
        response,
        Response::IfShell(rmux_proto::IfShellResponse::no_output())
    );
}

#[tokio::test]
async fn if_shell_string_mode_runs_multiple_commands_in_one_group() {
    let handler = RequestHandler::new();

    let response = handler
        .handle(Request::IfShell(IfShellRequest {
            condition: "1".to_owned(),
            format_mode: true,
            then_command: "set-buffer -b one first; set-buffer -b two second".to_owned(),
            else_command: None,
            target: None,
            caller_cwd: None,
            background: false,
        }))
        .await;

    assert_eq!(
        response,
        Response::IfShell(rmux_proto::IfShellResponse::no_output())
    );
    assert_eq!(
        handler
            .handle(Request::ShowBuffer(ShowBufferRequest {
                name: Some("one".to_owned()),
            }))
            .await
            .command_output()
            .expect("one buffer output")
            .stdout(),
        b"first"
    );
    assert_eq!(
        handler
            .handle(Request::ShowBuffer(ShowBufferRequest {
                name: Some("two".to_owned()),
            }))
            .await
            .command_output()
            .expect("two buffer output")
            .stdout(),
        b"second"
    );
}

#[tokio::test]
async fn if_shell_inserted_assignments_apply_before_parent_queue_tail() {
    let handler = RequestHandler::new();
    let parsed = CommandParser::new()
        .parse("if-shell -F 1 { FOO=bar } ; run-shell 'printf %s \"${FOO-unset}\"'")
        .expect("commands parse");

    let output = handler
        .execute_parsed_commands_for_test(std::process::id(), parsed)
        .await
        .expect("queue succeeds");

    assert_eq!(output.stdout(), b"bar");

    let state = handler.state.lock().await;
    assert_eq!(state.environment.global_value("FOO"), Some("bar"));
}
