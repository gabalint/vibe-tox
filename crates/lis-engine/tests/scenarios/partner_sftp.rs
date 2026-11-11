//! SFTP connections (ticket #4555 step 3; REQ-RPT-012): the LIS's own SSH key, a password
//! kept sealed and never returned, kept only for the same server and user, and a server
//! test that saves nothing. The transport itself is tested in `partner_delivery::sftp`.
//! Synthetic data only.

use lis_core::{CommandError, ErrorCode};
use lis_engine::{
    CreatePartnerConnectionInput, TestPartnerSftpInput, UpdatePartnerConnectionInput,
};
use lis_interfaces::partner::{
    PartnerConnection, PartnerConnectionDetail, PartnerConnectionSettings, PartnerMessageType,
    PartnerProtocol, Schedule, SftpAuth, SftpSettings,
};
use lis_store::testing::local_port;

use crate::fixture::{TestEngine, secret};

const SYNTHETIC_PASSWORD: &str = "synthetic-sftp-password";

fn server(port: u16) -> SftpSettings {
    SftpSettings {
        host: "127.0.0.1".to_owned(),
        port,
        username: "vibetox_lab".to_owned(),
        auth: SftpAuth::Password,
        fingerprint: format!("SHA256:{}", "A".repeat(43)),
        remote_folder: "/inbound/results".to_owned(),
    }
}

fn settings(sftp: SftpSettings) -> PartnerConnectionSettings {
    PartnerConnectionSettings {
        name: "HL7 Results - Synthetic clinic".to_owned(),
        partner: "SYN-CLINIC".to_owned(),
        message_type: PartnerMessageType::Hl7Oru,
        protocol: PartnerProtocol::Sftp,
        rules: Vec::new(),
        schedule: Schedule::Immediately,
        file_name_pattern: "{report}_r{revision}.hl7".to_owned(),
        sftp: Some(sftp),
        intake: None,
        mllp: None,
        mllp_listener: None,
    }
}

async fn create(
    t: &TestEngine,
    sftp: SftpSettings,
    password: Option<&str>,
) -> Result<PartnerConnectionDetail, CommandError> {
    t.engine
        .create_partner_connection(CreatePartnerConnectionInput {
            settings: settings(sftp),
            folder_choice: None,
            processed_folder_choice: None,
            failed_folder_choice: None,
            password: password.map(secret),
            active: false,
        })
        .await
}

/// Changes the connection's server without typing the password.
async fn change(
    t: &TestEngine,
    connection: &PartnerConnection,
    sftp: SftpSettings,
) -> Result<PartnerConnectionDetail, CommandError> {
    t.engine
        .update_partner_connection(UpdatePartnerConnectionInput {
            connection_id: connection.id,
            expected_revision: connection.revision,
            settings: settings(sftp),
            folder_choice: None,
            processed_folder_choice: None,
            failed_folder_choice: None,
            password: None,
            reason: "Synthetic server change".to_owned(),
        })
        .await
}

fn field_of(error: &CommandError) -> String {
    let json = serde_json::to_value(error).unwrap();
    json["fieldErrors"][0]["field"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[tokio::test]
async fn a_folder_connection_can_move_to_sftp() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let folder = tempfile::tempdir().unwrap();
    let chosen = t
        .engine
        .choose_partner_folder(Some(folder.path().to_owned()))
        .await
        .unwrap()
        .unwrap();
    let in_folder = PartnerConnectionSettings {
        protocol: PartnerProtocol::Folder,
        sftp: None,
        ..settings(server(local_port()))
    };
    let created = t
        .engine
        .create_partner_connection(CreatePartnerConnectionInput {
            settings: in_folder,
            folder_choice: Some(chosen.folder_choice),
            processed_folder_choice: None,
            failed_folder_choice: None,
            password: None,
            active: false,
        })
        .await
        .unwrap()
        .connection;
    let moved = t
        .engine
        .update_partner_connection(UpdatePartnerConnectionInput {
            connection_id: created.id,
            expected_revision: created.revision,
            settings: settings(server(local_port())),
            folder_choice: None,
            processed_folder_choice: None,
            failed_folder_choice: None,
            password: Some(secret(SYNTHETIC_PASSWORD)),
            reason: "Synthetic move to the partner's server".to_owned(),
        })
        .await
        .unwrap()
        .connection;
    assert_eq!(moved.settings.protocol, PartnerProtocol::Sftp);
    assert_eq!(moved.folder, "/inbound/results");
    assert!(moved.has_password);
}

#[tokio::test]
async fn the_lis_key_is_made_once_and_kept() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let key = t.engine.get_lis_sftp_key().await.unwrap();
    assert!(key.starts_with("ssh-ed25519 "), "{key}");
    assert!(key.ends_with(" openbga-tox"), "{key}");
    assert_eq!(t.engine.get_lis_sftp_key().await.unwrap(), key);
    let private: Vec<u8> = t
        .conn()
        .query_row("SELECT private_key FROM ifc_sftp_identity", [], |row| {
            row.get(0)
        })
        .unwrap();
    // Kept sealed: the stored bytes are not an OpenSSH private key.
    assert!(!String::from_utf8_lossy(&private).contains("OPENSSH PRIVATE KEY"));
}

#[tokio::test]
async fn a_password_is_sealed_never_returned_and_kept_only_for_the_same_server() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let port = local_port();
    let missing = create(&t, server(port), None).await.unwrap_err();
    assert_eq!(missing.code(), ErrorCode::Validation);
    assert_eq!(field_of(&missing), "settings.sftp.password");
    let connection = create(&t, server(port), Some(SYNTHETIC_PASSWORD))
        .await
        .unwrap()
        .connection;
    assert!(connection.has_password);
    assert_eq!(connection.folder, "/inbound/results");
    assert!(
        !serde_json::to_string(&connection)
            .unwrap()
            .contains(SYNTHETIC_PASSWORD)
    );
    let stored: Vec<u8> = t
        .conn()
        .query_row(
            "SELECT sftp_password FROM ifc_partner_connections",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!String::from_utf8_lossy(&stored).contains(SYNTHETIC_PASSWORD));

    // Another server needs the password again: a kept one never goes to another host.
    let moved = SftpSettings {
        host: "sftp.other-clinic.invalid".to_owned(),
        ..server(port)
    };
    let refused = change(&t, &connection, moved).await.unwrap_err();
    assert_eq!(field_of(&refused), "settings.sftp.password");

    // The same server with another folder keeps it.
    let other_folder = SftpSettings {
        remote_folder: "/inbound/other".to_owned(),
        ..server(port)
    };
    let changed = change(&t, &connection, other_folder).await.unwrap();
    assert!(changed.connection.has_password);
    assert_eq!(changed.connection.folder, "/inbound/other");

    // The kept password signs in to a test of the same server; nothing answers there.
    let tested = t
        .engine
        .test_partner_sftp(TestPartnerSftpInput {
            server: server(port),
            password: None,
            connection_id: Some(connection.id),
            intake: None,
        })
        .await
        .unwrap();
    assert_eq!(tested.problem.as_deref(), Some("server_unreachable"));
    assert_eq!(tested.presented_fingerprint, None);
}
