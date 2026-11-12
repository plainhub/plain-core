use super::{CLIENT_IDENTITY, ksf};
use crate::opaque::errors::ProtocolError;
use crate::opaque::{
    ClientRegistration, ClientRegistrationFinishParameters, CredentialFinalization,
    CredentialRequest, Identifiers, ServerLogin, ServerLoginParameters, ServerRegistration,
    ServerSetup,
};
use rand_core::OsRng;
use zeroize::Zeroizing;

/// Stored only in Rust-owned credential preferences.
pub struct Registration {
    pub setup: Zeroizing<Vec<u8>>,
    pub record: Zeroizing<Vec<u8>>,
}

pub fn register(password: &[u8], server_identity: &[u8]) -> Result<Registration, ProtocolError> {
    let setup = ServerSetup::new(&mut OsRng);
    let client = ClientRegistration::start(&mut OsRng, password)?;
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
            ServerRegistration::finish(upload.message)
                .serialize()
                .to_vec(),
        ),
    })
}

pub struct Login {
    state: ServerLogin,
}

pub fn start(
    setup: &[u8],
    record: &[u8],
    request: &[u8],
    context: &[u8],
    server_identity: &[u8],
) -> Result<(Login, Vec<u8>), ProtocolError> {
    let setup = ServerSetup::deserialize(setup)?;
    let record = ServerRegistration::deserialize(record)?;
    let request = CredentialRequest::deserialize(request)?;
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
            CredentialFinalization::deserialize(message)?,
            Default::default(),
        )?;
        Ok(Zeroizing::new(result.session_key.to_vec()))
    }
}

pub fn public_key(setup: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    Ok(ServerSetup::deserialize(setup)?
        .public_key()
        .serialize()
        .to_vec())
}

pub fn finalization_len() -> usize {
    64
}

pub fn validate_registration(setup: &[u8], record: &[u8]) -> Result<(), ProtocolError> {
    ServerSetup::deserialize(setup)?;
    ServerRegistration::deserialize(record)?;
    Ok(())
}
