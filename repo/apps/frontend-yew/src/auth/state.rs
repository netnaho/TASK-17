use std::collections::HashSet;
use std::rc::Rc;

use gloo::storage::{LocalStorage, Storage};
use serde::{Deserialize, Serialize};
use yew::prelude::*;

const STORAGE_KEY: &str = "silveroak.auth";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Principal {
    pub user_id: String,
    pub email: String,
    pub roles: Vec<String>,
    #[serde(default)]
    pub permissions: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AuthState {
    pub principal: Option<Principal>,
    pub api_token: Option<String>,
    pub signing_key: Option<String>,
    pub bootstrapped: bool,
}

impl AuthState {
    pub fn is_authenticated(&self) -> bool { self.principal.is_some() && self.api_token.is_some() }
    pub fn has_permission(&self, p: &str) -> bool {
        self.principal
            .as_ref()
            .map(|pr| pr.permissions.contains(p) || pr.permissions.contains("*"))
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub enum AuthAction {
    Bootstrap,
    SignedIn { principal: Principal, api_token: String, signing_key: String },
    SignedOut,
}

impl Reducible for AuthState {
    type Action = AuthAction;
    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        let next = match action {
            AuthAction::Bootstrap => {
                let stored: Option<PersistedAuth> = LocalStorage::get(STORAGE_KEY).ok();
                if let Some(s) = stored {
                    AuthState {
                        principal: Some(s.principal),
                        api_token: Some(s.api_token),
                        signing_key: Some(s.signing_key),
                        bootstrapped: true,
                    }
                } else {
                    AuthState { bootstrapped: true, ..Default::default() }
                }
            }
            AuthAction::SignedIn { principal, api_token, signing_key } => {
                let _ = LocalStorage::set(
                    STORAGE_KEY,
                    PersistedAuth {
                        principal: principal.clone(),
                        api_token: api_token.clone(),
                        signing_key: signing_key.clone(),
                    },
                );
                AuthState {
                    principal: Some(principal),
                    api_token: Some(api_token),
                    signing_key: Some(signing_key),
                    bootstrapped: true,
                }
            }
            AuthAction::SignedOut => {
                LocalStorage::delete(STORAGE_KEY);
                AuthState { bootstrapped: true, ..Default::default() }
            }
        };
        Rc::new(next)
    }
}

#[derive(Serialize, Deserialize)]
struct PersistedAuth {
    principal: Principal,
    api_token: String,
    signing_key: String,
}

pub type AuthContext = UseReducerHandle<AuthState>;

#[derive(Properties, PartialEq)]
pub struct AuthProviderProps { pub children: Children }

#[function_component(AuthProvider)]
pub fn auth_provider(props: &AuthProviderProps) -> Html {
    let state = use_reducer(AuthState::default);
    {
        let state = state.clone();
        use_effect_with((), move |_| {
            state.dispatch(AuthAction::Bootstrap);
            || ()
        });
    }
    html! {
        <ContextProvider<AuthContext> context={state}>
            { for props.children.iter() }
        </ContextProvider<AuthContext>>
    }
}

#[hook]
pub fn use_auth() -> AuthContext {
    use_context::<AuthContext>().expect("AuthProvider missing")
}
