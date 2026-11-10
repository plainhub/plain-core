use super::{CLIENT_IDENTITY, Suite, ksf};
use opaque_ke::errors::ProtocolError;
use opaque_ke::{
    ClientRegistration, ClientRegistrationFinishParameters, CredentialFinalization,
    CredentialRequest, Identifiers, ServerLogin, ServerLoginParameters, ServerRegistration,
    ServerSetup,
};
use rand_core::OsRng;
use zeroize::Zeroizing;

/// Stored only in the existing protected password preference.
pub struct Registration {
    pub setup: Zeroizing<Vec<u8>>,
    pub record: Zeroizing<Vec<u8>>,
}

pub fn register(password: &[u8], server_identity: &[u8]) -> Result<Registration, ProtocolError> {
    let setup = ServerSetup::<Suite>::new(&mut OsRng);
    let client = ClientRegistration::<Suite>::start(&mut OsRng, password)?;
    let response = ServerRegistration::start(&setup, client.message, CLIENT_IDENTITY)?;
    let stretched = ksf();
    let upload = client.state.finish(
        &mut OsRng,
        password,
        response.message,
        ClientRegistrationFinishParameters::new(
            Identifiers {
                client: Some(CLIENT_IDENTITY),
                server: Some(server_identity),
            },
            Some(&stretched),
        ),
    )?;
    Ok(Registration {
        setup: Zeroizing::new(setup.serialize().to_vec()),
        record: Zeroizing::new(
            ServerRegistration::<Suite>::finish(upload.message)
                .serialize()
                .to_vec(),
        ),
    })
}

pub struct Login {
    state: ServerLogin<Suite>,
}

pub fn start(
    setup: &[u8],
    record: &[u8],
    request: &[u8],
    context: &[u8],
    server_identity: &[u8],
) -> Result<(Login, Vec<u8>), ProtocolError> {
    let setup = ServerSetup::<Suite>::deserialize(setup)?;
    let record = ServerRegistration::<Suite>::deserialize(record)?;
    let request = CredentialRequest::<Suite>::deserialize(request)?;
    let result = ServerLogin::start(
        &mut OsRng,
        &setup,
        Some(record),
        request,
        CLIENT_IDENTITY,
        ServerLoginParameters {
            context: Some(context),
            identifiers: Identifiers {
                client: Some(CLIENT_IDENTITY),
                server: Some(server_identity),
            },
        },
    )?;
    Ok((
        Login {
            state: result.state,
        },
        result.message.serialize().to_vec(),
    ))
}

impl Login {
    pub fn finish(self, message: &[u8]) -> Result<Zeroizing<Vec<u8>>, ProtocolError> {
        let result = self.state.finish(
            CredentialFinalization::<Suite>::deserialize(message)?,
            Default::default(),
        )?;
        Ok(Zeroizing::new(result.session_key.to_vec()))
    }
}

pub fn public_key(setup: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    Ok(ServerSetup::<Suite>::deserialize(setup)?
        .keypair()
        .public()
        .serialize()
        .to_vec())
}

pub fn finalization_len() -> usize {
    opaque_ke::generic_array::GenericArray::<u8, opaque_ke::CredentialFinalizationLen<Suite>>::default().len()
}
