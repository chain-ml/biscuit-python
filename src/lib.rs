use biscuit_auth as biscuit;
use hex;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::*;

use pyo3::create_exception;

create_exception!(biscuit_auth, DataLogError, pyo3::exceptions::PyException);
create_exception!(biscuit_auth, AuthorizationError, pyo3::exceptions::PyException);
create_exception!(biscuit_auth, BiscuitBuildError, pyo3::exceptions::PyException);
create_exception!(biscuit_auth, BiscuitValidationError, pyo3::exceptions::PyException);
create_exception!(biscuit_auth, BiscuitSerializationError, pyo3::exceptions::PyException);

impl std::convert::From<biscuit::error::Token> for PyErr {
    fn from(err: biscuit::error::Token) -> PyErr {
        PyValueError::new_err(err.to_string())
    }
}

#[pyclass(name="BiscuitBuilder")]
pub struct PyBiscuitBuilder {
    facts: Vec<biscuit::builder::Fact>,
    rules: Vec<biscuit::builder::Rule>,
    checks: Vec<biscuit::builder::Check>,
}

#[pymethods]
impl PyBiscuitBuilder {

    #[new]
    fn new() -> PyBiscuitBuilder {
        PyBiscuitBuilder {
            facts: Vec::new(),
            rules: Vec::new(),
            checks: Vec::new(),
        }
    }

    pub fn build(&self, root: &PyKeyPair) -> PyResult<PyBiscuit> {
        let mut builder = biscuit::Biscuit::builder(&root.0);

        for fact in &self.facts {
            match builder.add_authority_fact(fact.clone()) {
                Ok(_) => Ok(()),
                Err(error) => return Err(BiscuitBuildError::new_err(error.to_string()))
            }
        }
        for rule in &self.rules {
            match builder.add_authority_rule(rule.clone()) {
                Ok(_) => Ok(()),
                Err(error) => return Err(BiscuitBuildError::new_err(error.to_string()))
            }
        }
        for check in &self.checks {
            match builder.add_authority_check(check.clone()) {
                Ok(_) => Ok(()),
                Err(error) => return Err(BiscuitBuildError::new_err(error.to_string()))
            }
        }

        match builder.build() {
            Ok(biscuit) => Ok(PyBiscuit(biscuit)),
            Err(error) => Err(BiscuitBuildError::new_err(error.to_string()))
        }
    }

    /// Adds a Datalog fact
    pub fn add_authority_fact(&mut self, fact: &str) -> PyResult<()> {
        match fact.try_into() {
            Ok(fact) => Ok(self.facts.push(fact)),
            Err(error) => Err(DataLogError::new_err(error.to_string())),
        }
    }

    /// Adds a Datalog rule
    pub fn add_authority_rule(&mut self, rule: &str) -> PyResult<()> {
        match rule.try_into() {
            Ok(rule) => Ok(self.rules.push(rule)),
            Err(error) => Err(DataLogError::new_err(error.to_string())),
        }
    }

    /// Adds a check
    ///
    /// All checks, from authorizer and token, must be validated to authorize the request
    pub fn add_authority_check(&mut self, check: &str) -> PyResult<()> {
        match check.try_into() {
            Ok(check) => Ok(self.checks.push(check)),
            Err(error) => Err(DataLogError::new_err(error.to_string())),
        }
    }
}

#[pyclass(name="Biscuit")]
pub struct PyBiscuit(biscuit::Biscuit);

#[pymethods]
impl PyBiscuit {
    /// Creates a BiscuitBuilder
    ///
    /// the builder can then create a new token with a root key
    #[staticmethod]
    pub fn builder() -> PyBiscuitBuilder {
        PyBiscuitBuilder::new()
    }

    /// Creates a BlockBuilder to prepare for attenuation
    ///
    /// the bulder can then be given to the token's append method to create an attenuated token
    pub fn create_block(&self) -> PyBlockBuilder {
        PyBlockBuilder(self.0.create_block())
    }

    /// Creates an attenuated token by adding the block generated by the BlockBuilder
    pub fn append(&self, block: PyBlockBuilder) -> PyResult<PyBiscuit> {
        match self.0.append(block.0) {
            Ok(biscuit) => Ok(PyBiscuit(biscuit)),
            Err(error) => Err(BiscuitBuildError::new_err(error.to_string()))
        }
    }

    /// Creates an authorizer from the token
    pub fn authorizer(&self) -> PyAuthorizer {
        PyAuthorizer {
            token: Some(self.0.clone()),
            ..PyAuthorizer::default()
        }
    }

    /// Seals the token
    ///
    /// A sealed token cannot be attenuated
    pub fn seal(&self) -> PyBiscuit {
        PyBiscuit(
            self.0
                .seal().unwrap(),
        )
    }

    /// Deserializes a token from raw data
    ///
    /// This will check the signature using the root key
    #[classmethod]
    pub fn from_bytes(_: &PyType, data: &[u8], root: &PyPublicKey) -> PyResult<PyBiscuit> {
        match biscuit::Biscuit::from(data, |_| root.0) {
            Ok(biscuit) => Ok(PyBiscuit(biscuit)),
            Err(error) => Err(BiscuitValidationError::new_err(error.to_string()))
        }
    }

    /// Deserializes a token from URL safe base 64 data
    ///
    /// This will check the signature using the root key
    /// 
    #[classmethod]
    pub fn from_base64(_: &PyType, data: &[u8], root: &PyPublicKey) -> PyResult<PyBiscuit> {
        match biscuit::Biscuit::from_base64(data, |_| root.0) {
            Ok(biscuit) => Ok(PyBiscuit(biscuit)),
            Err(error) => Err(BiscuitValidationError::new_err(error.to_string()))
        }
    }

    /// Serializes to raw data
    pub fn to_bytes(&self) -> PyResult<Vec<u8>> {
        match self.0.to_vec() {
            Ok(vec) => Ok(vec),
            Err(error) => Err(BiscuitSerializationError::new_err(error.to_string()))
        }
    }

    /// Serializes to URL safe base 64 data
    pub fn to_base64(&self) -> String {
        self.0.to_base64().unwrap()
    }

    // TODO Revocation IDs

    /// Returns the number of blocks in the token
    pub fn block_count(&self) -> usize {
        self.0.block_count()
    }

    /// Prints a block's content as Datalog code
    pub fn block_source(&self, index: usize) -> Option<String> {
        self.0.print_block_source(index)
    }

    fn __repr__(&self) -> String {
        self.0.print()
    }
}

/// The Authorizer verifies a request according to its policies and the provided token
#[pyclass(name="Authorizer")]
#[derive(Default)]
pub struct PyAuthorizer {
    token: Option<biscuit::Biscuit>,
    facts: Vec<biscuit::builder::Fact>,
    rules: Vec<biscuit::builder::Rule>,
    checks: Vec<biscuit::builder::Check>,
    policies: Vec<biscuit::builder::Policy>,
}

#[pymethods]
impl PyAuthorizer {
    #[new]
    pub fn new() -> PyAuthorizer {
        PyAuthorizer::default()
    }

    /// Adds a Datalog fact
    pub fn add_fact(&mut self, fact: &str) -> PyResult<()> {
        match fact.try_into() {
            Ok(fact) => Ok(self.facts.push(fact)),
            Err(error) => Err(DataLogError::new_err(error.to_string())),
        }
    }

    /// Adds a Datalog rule
    pub fn add_rule(&mut self, rule: &str) -> PyResult<()> {
        match rule.try_into() {
            Ok(rule) => Ok(self.rules.push(rule)),
            Err(error) => Err(DataLogError::new_err(error.to_string())),
        }
    }

    /// Adds a check
    ///
    /// All checks, from authorizer and token, must be validated to authorize the request
    pub fn add_check(&mut self, check: &str) -> PyResult<()> {
        match check.try_into() {
            Ok(check) => Ok(self.checks.push(check)),
            Err(error) => Err(DataLogError::new_err(error.to_string())),
        }
    }

    /// Adds a policy
    ///
    /// The authorizer will test all policies in order of addition and stop at the first one that
    /// matches. If it is a "deny" policy, the request fails, while with an "allow" policy, it will
    /// succeed
    pub fn add_policy(&mut self, policy: &str) -> PyResult<()> {
        match policy.try_into() {
            Ok(policy) => Ok(self.policies.push(policy)),
            Err(error) => Err(DataLogError::new_err(error.to_string())),
        }
    }

    /// Adds facts, rules, checks and policies as one code block
    pub fn add_code(&mut self, source: &str) -> PyResult<()> {
        let source_result = match biscuit::parser::parse_source(source) {
            Ok(source_result) => source_result,
            // We're only returning the first error here (because we can only raise one exception)
            Err(error) => return Err(DataLogError::new_err(error[0].to_string())),
        };

        for (_, fact) in source_result.facts.into_iter() {
            self.facts.push(fact);
        }

        for (_, rule) in source_result.rules.into_iter() {
            self.rules.push(rule);
        }

        for (_, check) in source_result.checks.into_iter() {
            self.checks.push(check);
        }

        for (_, policy) in source_result.policies.into_iter() {
            self.policies.push(policy);
        }

        Ok(())
    }

    /// Runs the authorization checks and policies
    ///
    /// Returns the index of the matching allow policy, or an error containing the matching deny
    /// policy or a list of the failing checks
    pub fn authorize(&self) -> PyResult<usize> {
        let mut authorizer = match &self.token {
            Some(token) => token
                .authorizer().unwrap(),
            None => biscuit::Authorizer::new().unwrap(),
        };

        for fact in self.facts.iter() {
            authorizer.add_fact(fact.clone()).unwrap();
        }
        for rule in self.rules.iter() {
            authorizer
                .add_rule(rule.clone()).unwrap();
        }
        for check in self.checks.iter() {
            authorizer
                .add_check(check.clone()).unwrap();
        }
        for policy in self.policies.iter() {
            authorizer
                .add_policy(policy.clone()).unwrap();
        }

        match authorizer.authorize() {
            Ok(policy_index) => Ok(policy_index),
            Err(error) => Err(AuthorizationError::new_err(error.to_string()))
        }
    }
}

/// Creates a block to attenuate a token
#[pyclass(name="BlockBuilder")]
#[derive(Clone)]
pub struct PyBlockBuilder(biscuit::builder::BlockBuilder);

#[pymethods]
impl PyBlockBuilder {
    /// Adds a Datalog fact
    pub fn add_fact(&mut self, fact: &str) -> PyResult<()> {
        match self.0.add_fact(fact) {
            Ok(_) => Ok(()),
            Err(error) => Err(DataLogError::new_err(error.to_string()))
        }
    }

    /// Adds a Datalog rule
    pub fn add_rule(&mut self, rule: &str) ->  PyResult<()> {
        match self.0.add_rule(rule) {
            Ok(_) => Ok(()),
            Err(error) => Err(DataLogError::new_err(error.to_string()))
        }
    }

    /// Adds a check
    ///
    /// All checks, from authorizer and token, must be validated to authorize the request
    pub fn add_check(&mut self, check: &str) -> PyResult<()> {
        match self.0.add_check(check) {
            Ok(_) => Ok(()),
            Err(error) => Err(DataLogError::new_err(error.to_string()))
        }
    }

    /// Adds facts, rules, checks and policies as one code block
    pub fn add_code(&mut self, source: &str) -> PyResult<()> {
        match self.0.add_code(source) {
            Ok(_) => Ok(()),
            Err(error) => Err(DataLogError::new_err(error.to_string()))
        }
    }
}

#[pyclass(name="KeyPair")]
pub struct PyKeyPair(biscuit::KeyPair);

#[pymethods]
impl PyKeyPair {
    #[new]
    pub fn new() -> Self {
        PyKeyPair(biscuit::KeyPair::new())
    }

    #[classmethod]
    pub fn from_existing(_: &PyType, private_key: PyPrivateKey) -> Self {
        PyKeyPair(biscuit::KeyPair::from(private_key.0))
    }

    #[getter]
    pub fn public_key(&self) -> PyPublicKey {
        PyPublicKey(self.0.public())
    }

    #[getter]
    pub fn private_key(&self) -> PyPrivateKey {
        PyPrivateKey(self.0.private())
    }
}

/// Public key
#[pyclass(name="PublicKey")]
pub struct PyPublicKey(biscuit::PublicKey);

#[pymethods]
impl PyPublicKey {
    /// Serializes a public key to raw bytes
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Serializes a public key to a hexadecimal string
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0.to_bytes())
    }

    /// Deserializes a public key from raw bytes
    #[classmethod]
    pub fn from_bytes(_: &PyType, data: &[u8]) -> PyResult<PyPublicKey> {
        match biscuit::PublicKey::from_bytes(data) {
            Ok(key) => Ok(PyPublicKey(key)),
            Err(error) => Err(PyValueError::new_err(error.to_string())),
        }
    }

    /// Deserializes a public key from a hexadecimal string
    #[classmethod]
    pub fn from_hex(_: &PyType, data: &str) -> PyResult<PyPublicKey> {
        let data = match hex::decode(data) {
            Ok(data) => data,
            Err(error) => return Err(PyValueError::new_err(error.to_string())),
        };
        match biscuit::PublicKey::from_bytes(&data) {
            Ok(key) => Ok(PyPublicKey(key)),
            Err(error) => Err(PyValueError::new_err(error.to_string())),
        }
    }
}

#[pyclass(name="PrivateKey")]
#[derive(Clone)]
pub struct PyPrivateKey(biscuit::PrivateKey);

#[pymethods]
impl PyPrivateKey {
    /// Serializes a private key to raw bytes
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0.to_bytes()
    }

    /// Serializes a private key to a hexadecimal string
    pub fn to_hex(&self) -> String {
        hex::encode(&self.0.to_bytes())
    }

    /// Deserializes a private key from raw bytes
    #[classmethod]
    pub fn from_bytes(_: &PyType, data: &[u8]) -> PyResult<PyPrivateKey> {
        match biscuit::PrivateKey::from_bytes(data) {
            Ok(key) => Ok(PyPrivateKey(key)),
            Err(error) => Err(PyValueError::new_err(error.to_string())),
        }
    }

    /// Deserializes a private key from a hexadecimal string
    #[classmethod]
    pub fn from_hex(_: &PyType, data: &str) -> PyResult<PyPrivateKey> {
        let data = match hex::decode(data) {
            Ok(data) => data,
            Err(error) => return Err(PyValueError::new_err(error.to_string())),
        };
        match biscuit::PrivateKey::from_bytes(&data) {
            Ok(key) => Ok(PyPrivateKey(key)),
            Err(error) => Err(PyValueError::new_err(error.to_string())),
        }
    }
}


#[pymodule]
fn biscuit_auth(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyKeyPair>()?;
    m.add_class::<PyPublicKey>()?;
    m.add_class::<PyPrivateKey>()?;
    m.add_class::<PyBiscuit>()?;
    m.add_class::<PyBiscuitBuilder>()?;
    m.add_class::<PyBlockBuilder>()?;

    m.add("DataLogError", py.get_type::<DataLogError>())?;
    m.add("AuthorizationError", py.get_type::<AuthorizationError>())?;
    m.add("BiscuitBuildError", py.get_type::<BiscuitBuildError>())?;
    m.add("BiscuitValidationError", py.get_type::<BiscuitValidationError>())?;
    m.add("BiscuitSerializationError", py.get_type::<BiscuitSerializationError>())?;

    Ok(())
}