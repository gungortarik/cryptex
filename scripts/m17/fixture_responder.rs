//! Disposable responder for the historical Mac UI. Input and output are public
//! pairing messages. This example links the historical event crate, not cryptexd.
use cryptex_core::DeviceId;
use cryptex_events::{
    create_bootstrap_proof, sign_bootstrap_approval, verify_bootstrap_approval_request,
    verify_bootstrap_offer, verify_bootstrap_transcript, BootstrapApprovalRole,
};
use ed25519_dalek::SigningKey;
use std::io::{self, Read, Write};

const PEER: &str = "44444444-4444-4444-8444-444444444444";
const TRANSPORT: &str = "XAAPCAD-GA4SF4J-BNQL3FS-T456WKV-FGLXUKO-CDWHVDW-EYWNRIG-SM5GUAZ";

fn reply(
    messages: &[&[u8]],
    brain: &str,
    device: &str,
    public: &str,
    transport: &str,
) -> Option<Vec<u8>> {
    let offer = verify_bootstrap_offer(messages.first()?).ok()?;
    let initiator = offer.initiator();
    let encoded = initiator
        .public_key
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    if offer.brain_id() != brain
        || initiator.device_id.as_str() != device
        || encoded != public
        || offer.initiator_transport_device_id() != transport
    {
        return None;
    }
    let key = SigningKey::from_bytes(&[35; 32]);
    let proof = create_bootstrap_proof(
        messages[0],
        &DeviceId::new(PEER).ok()?,
        TRANSPORT,
        &[2; 32],
        &key,
    )
    .ok()?;
    match messages {
        [_] => Some(proof),
        [offer, received, first] if *received == proof => {
            let prefix =
                verify_bootstrap_approval_request(messages, BootstrapApprovalRole::Responder)
                    .ok()?;
            let last = sign_bootstrap_approval(&prefix, &key).ok()?;
            verify_bootstrap_transcript(&[offer, received, first, &last]).ok()?;
            Some(last)
        }
        _ => None,
    }
}

fn main() -> Result<(), &'static str> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [brain, device, public, transport] = args.as_slice() else {
        return Err("fixture identity required");
    };
    let mut input = String::new();
    io::stdin()
        .take(196609)
        .read_to_string(&mut input)
        .map_err(|_| "fixture input")?;
    if input.len() > 196608 {
        return Err("fixture input limit");
    }
    let messages: Vec<_> = input.lines().map(str::as_bytes).collect();
    let result =
        reply(&messages, brain, device, public, transport).ok_or("fixture transcript refused")?;
    io::stdout()
        .write_all(&result)
        .map_err(|_| "fixture output")
}

#[cfg(test)]
mod tests {
    use super::*;
    use cryptex_events::create_bootstrap_offer;

    #[test]
    fn requires_the_enrolled_initiator_and_its_exact_signed_prefix() {
        let brain = "11111111-1111-4111-8111-111111111111";
        let device = DeviceId::new("55555555-5555-4555-8555-555555555555").unwrap();
        let local = "U2SYIGU-YWZX757-2WPZGCR-DHD7MLH-ZKV7HLJ-EYO6CXE-K3DU3DU-SDFXYAE";
        let key = SigningKey::from_bytes(&[17; 32]);
        let public = key
            .verifying_key()
            .to_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let offer =
            create_bootstrap_offer(brain, &"b".repeat(64), &device, local, &[1; 32], &key).unwrap();
        let answer = |messages: &[&[u8]]| reply(messages, brain, device.as_str(), &public, local);
        let proof = answer(&[&offer]).unwrap();
        let prefix = [&offer[..], &proof[..]];
        let approval = sign_bootstrap_approval(
            &verify_bootstrap_approval_request(&prefix, BootstrapApprovalRole::Initiator).unwrap(),
            &key,
        )
        .unwrap();
        let last = answer(&[&offer, &proof, &approval]).unwrap();
        verify_bootstrap_transcript(&[&offer, &proof, &approval, &last]).unwrap();
        assert!(reply(&[&offer], brain, PEER, &public, local).is_none());
        assert!(reply(&[&offer], brain, device.as_str(), &"00".repeat(32), local).is_none());
        assert!(reply(&[&offer], brain, device.as_str(), &public, TRANSPORT).is_none());
        assert!(answer(&[&offer, &proof, &proof]).is_none());
        assert!(answer(&[&offer, &approval, &approval]).is_none());
    }
}
