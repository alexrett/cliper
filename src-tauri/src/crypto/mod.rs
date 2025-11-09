use anyhow::{anyhow, Result};
use ring::aead::{Aad, LessSafeKey, UnboundKey, AES_256_GCM, Nonce};
use ring::rand::{SecureRandom, SystemRandom};
use security_framework::passwords::{get_generic_password, set_generic_password, delete_generic_password};
use zeroize::{Zeroize, Zeroizing};

const SERVICE_SUFFIX: &str = ".masterkey";
const ACCOUNT: &str = "default";
const KEY_LEN: usize = 32; // 256-bit
const NONCE_LEN: usize = 12; // 96-bit IV for AES-GCM

pub struct KeyManager {
    bundle_id: String,
    // Raw key bytes stored when unlocked; zeroized on lock.
    key: parking_lot::Mutex<Option<Zeroizing<Vec<u8>>>>,
    rng: SystemRandom,
}

impl KeyManager {
    pub fn new(bundle_id: String) -> Self {
        Self {
            bundle_id,
            key: parking_lot::Mutex::new(None),
            rng: SystemRandom::new(),
        }
    }

    fn service_name(&self) -> String {
        format!("{}{}", self.bundle_id, SERVICE_SUFFIX)
    }

    pub fn is_unlocked(&self) -> bool {
        self.key.lock().is_some()
    }

    pub fn lock(&self) {
        let mut guard = self.key.lock();
        if let Some(mut k) = guard.take() {
            k.zeroize();
        }
    }

    pub fn reset_master_key(&self) -> Result<()> {
        let service = self.service_name();
        let _ = delete_generic_password(&service, ACCOUNT); // ignore error if not exists
        let mut key = vec![0u8; KEY_LEN];
        self.rng
            .fill(&mut key)
            .map_err(|_| anyhow!("rng failed"))?;
        set_generic_password(&service, ACCOUNT, &key)?;
        let z = Zeroizing::from(key);
        *self.key.lock() = Some(z);
        Ok(())
    }

    pub fn unlock(&self) -> Result<()> {
        // Try to load from Keychain; if missing, generate and store.
        let service = self.service_name();
        let existing = get_generic_password(&service, ACCOUNT).ok();
        let key = match existing {
            Some(bytes) => bytes,
            None => {
                let mut key = vec![0u8; KEY_LEN];
                self.rng
                    .fill(&mut key)
                    .map_err(|_| anyhow!("rng failed"))?;
                set_generic_password(&service, ACCOUNT, &key)?;
                key
            }
        };
        let z = Zeroizing::from(key);
        *self.key.lock() = Some(z);
        Ok(())
    }

    fn less_safe_key(&self) -> Result<LessSafeKey> {
        let guard = self.key.lock();
        let key = guard.as_ref().ok_or_else(|| anyhow!("locked"))?;
        let unbound = UnboundKey::new(&AES_256_GCM, key).map_err(|_| anyhow!("bad key"))?;
        Ok(LessSafeKey::new(unbound))
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let key = self.less_safe_key()?;
        let mut nonce = [0u8; NONCE_LEN];
        self.rng
            .fill(&mut nonce)
            .map_err(|_| anyhow!("rng failed"))?;
        // buffer: nonce || ciphertext+tag
        let mut buf = Vec::with_capacity(NONCE_LEN + plaintext.len() + AES_256_GCM.tag_len());
        buf.extend_from_slice(&nonce);
        buf.extend_from_slice(plaintext);
        // offset not needed; we split after NONCE_LEN
        let mut slice = buf.split_off(NONCE_LEN);
        let nonce = Nonce::assume_unique_for_key(nonce);
        key.seal_in_place_append_tag(nonce, Aad::empty(), &mut slice)
            .map_err(|_| anyhow!("encrypt failed"))?;
        let mut out = Vec::with_capacity(NONCE_LEN + slice.len());
        out.extend_from_slice(&buf[..NONCE_LEN]);
        out.extend_from_slice(&slice);
        Ok(out)
    }

    pub fn decrypt(&self, blob: &[u8]) -> Result<Vec<u8>> {
        if blob.len() < NONCE_LEN + AES_256_GCM.tag_len() {
            return Err(anyhow!("blob too short"));
        }
        let key = self.less_safe_key()?;
        let nonce_bytes: [u8; NONCE_LEN] = blob[..NONCE_LEN]
            .try_into()
            .expect("slice with correct length");
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        let mut ciphertext = blob[NONCE_LEN..].to_vec();
        let out = key
            .open_in_place(nonce, Aad::empty(), &mut ciphertext)
            .map_err(|_| anyhow!("decrypt failed"))?;
        Ok(out.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let km = KeyManager::new("test.bundle".into());
        km.unlock().unwrap();
        let msg = b"hello world";
        let ct = km.encrypt(msg).unwrap();
        let pt = km.decrypt(&ct).unwrap();
        assert_eq!(pt, msg);
    }

    #[test]
    fn tamper_detected() {
        let km = KeyManager::new("test.bundle".into());
        km.unlock().unwrap();
        let msg = b"hello world";
        let mut ct = km.encrypt(msg).unwrap();
        // flip a bit
        let last = ct.len() - 1;
        ct[last] ^= 0x01;
        assert!(km.decrypt(&ct).is_err());
    }
}
