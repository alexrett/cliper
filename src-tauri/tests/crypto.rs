use cliper_lib::crypto::KeyManager;

#[test]
fn crypto_roundtrip_and_tamper() {
    let km = KeyManager::new("test.bundle".into());
    km.unlock().unwrap();
    let msg = b"secret message";
    let mut ct = km.encrypt(msg).unwrap();
    let pt = km.decrypt(&ct).unwrap();
    assert_eq!(pt, msg);

    // tamper
    let last = ct.len() - 2;
    ct[last] ^= 0xFF;
    assert!(km.decrypt(&ct).is_err());
}

