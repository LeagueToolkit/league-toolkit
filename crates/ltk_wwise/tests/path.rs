use ltk_wwise::{
    path::{AudioFileKind, AudioPath},
    BankId,
};

#[test]
fn voice_over_package_is_localized_and_registers_no_events() {
    let path = AudioPath::new(
        "ASSETS/Sounds/Wwise2016/VO/en_US/Characters/Ahri/Skins/Base/Ahri_Base_VO_audio.wpk",
    );

    assert_eq!(path.kind(), AudioFileKind::Package);
    assert!(path.is_localized());
    assert!(!path.registers_events());
}

#[test]
fn sfx_event_bank_is_shared_registers_events_and_hashes_its_file_name() {
    let path = AudioPath::new(
        "ASSETS/Sounds/Wwise2016/SFX/Characters/Ahri/Skins/Base/Ahri_Base_SFX_events.bnk",
    );

    assert_eq!(path.kind(), AudioFileKind::Bank);
    assert!(!path.is_localized());
    assert!(path.registers_events());
    assert_eq!(
        path.bank_id(),
        BankId::from_name("Ahri_Base_SFX_events.bnk")
    );
}

#[test]
fn windows_separators_give_the_same_bank_id_as_forward_slashes() {
    let windows = AudioPath::new(r"ASSETS\Sounds\Wwise2016\SFX\Shared\sett_base_sfx_audio.bnk");
    let forward = AudioPath::new("ASSETS/Sounds/Wwise2016/SFX/Shared/sett_base_sfx_audio.bnk");

    assert_eq!(windows.bank_id(), forward.bank_id());
    assert_eq!(windows.bank_id(), BankId(0xE9B7_0B40));
}

#[test]
fn package_extension_compares_case_insensitively() {
    assert_eq!(
        AudioPath::new("ASSETS/Sounds/Wwise2016/VO/EN_US/Ahri_Base_VO_audio.WPK").kind(),
        AudioFileKind::Package
    );
    assert_eq!(AudioPath::new("wpk").kind(), AudioFileKind::Bank);
}
