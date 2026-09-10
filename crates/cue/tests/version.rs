use assert_cmd::Command;

#[test]
fn reports_spike_version() {
    Command::cargo_bin("cue")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout("cue 0.2.0-spike\n");
}
