//! Authentication UI components

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

// Server functions - these work on both SSR and hydrate
#[server]
pub async fn login_user(email: String, password: String) -> Result<LoginResponse, ServerFnError> {
    use chai_core::auth::{self, AuthConfig};

    let config = AuthConfig::from_env().map_err(|e| ServerFnError::new(e.to_string()))?;

    match auth::login(&email, &password, &config.jwt_secret).await {
        Ok((user, token)) => {
            // Success is already logged in auth::login
            Ok(LoginResponse {
                user_id: user.id,
                email: user.email,
                token,
            })
        }
        Err(e) => {
            tracing::warn!(
                email = %email,
                error = %e,
                "Failed login attempt"
            );
            Err(ServerFnError::new(e.to_string()))
        }
    }
}

#[server]
pub async fn register_user(email: String, password: String) -> Result<(), ServerFnError> {
    use chai_core::auth;

    match auth::register(&email, &password).await {
        Ok(_) => {
            // Success is already logged in auth::register
            Ok(())
        }
        Err(e) => {
            tracing::warn!(
                email = %email,
                error = %e,
                "Failed registration attempt"
            );
            Err(ServerFnError::new(e.to_string()))
        }
    }
}

/// Login response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub user_id: i64,
    pub email: String,
    pub token: String,
}

/// Auth state stored in context
#[derive(Debug, Clone, Default)]
pub struct AuthState {
    pub token: Option<String>,
    pub email: Option<String>,
    pub user_id: Option<i64>,
}

impl AuthState {
    pub fn is_authenticated(&self) -> bool {
        self.token.is_some()
    }
}

/// Provide auth context for the entire app
#[component]
pub fn AuthProvider(children: Children) -> impl IntoView {
    let (auth_state, set_auth_state) = signal(AuthState::default());

    // Load token from localStorage on mount
    Effect::new(move |_| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("auth_token") {
                        if let Ok(Some(email)) = storage.get_item("auth_email") {
                            let user_id = storage
                                .get_item("auth_user_id")
                                .ok()
                                .flatten()
                                .and_then(|s| s.parse().ok());
                            set_auth_state.set(AuthState {
                                token: Some(token),
                                email: Some(email),
                                user_id,
                            });
                        }
                    }
                }
            }
        }
    });

    provide_context(auth_state);
    provide_context(set_auth_state);

    children()
}

/// Get auth read signal from context
pub fn use_auth() -> ReadSignal<AuthState> {
    expect_context::<ReadSignal<AuthState>>()
}

/// Get auth write signal from context
pub fn use_set_auth() -> WriteSignal<AuthState> {
    expect_context::<WriteSignal<AuthState>>()
}

/// Save auth to localStorage
#[allow(unused_variables)]
fn save_auth_to_storage(state: &AuthState) {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Some(token) = &state.token {
                    let _ = storage.set_item("auth_token", token);
                } else {
                    let _ = storage.remove_item("auth_token");
                }
                if let Some(email) = &state.email {
                    let _ = storage.set_item("auth_email", email);
                } else {
                    let _ = storage.remove_item("auth_email");
                }
                if let Some(user_id) = state.user_id {
                    let _ = storage.set_item("auth_user_id", &user_id.to_string());
                } else {
                    let _ = storage.remove_item("auth_user_id");
                }
            }
        }
    }
}

/// Login form component
#[component]
pub fn LoginForm(
    #[prop(optional)] on_success: Option<Callback<()>>,
    #[prop(optional)] on_switch_to_register: Option<Callback<()>>,
) -> impl IntoView {
    let set_auth = use_set_auth();
    let (email, set_email) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);
    let (loading, set_loading) = signal(false);

    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();

        let email_val = email.get();
        let password_val = password.get();

        if email_val.trim().is_empty() || password_val.is_empty() {
            set_error.set(Some("Заполните все поля".to_string()));
            return;
        }

        set_loading.set(true);
        set_error.set(None);

        leptos::task::spawn_local(async move {
            match login_user(email_val, password_val).await {
                Ok(response) => {
                    let state = AuthState {
                        token: Some(response.token),
                        email: Some(response.email),
                        user_id: Some(response.user_id),
                    };
                    save_auth_to_storage(&state);
                    set_auth.set(state);
                    if let Some(callback) = on_success {
                        callback.run(());
                    }
                }
                Err(e) => {
                    set_error.set(Some(e.to_string()));
                }
            }
            set_loading.set(false);
        });
    };

    view! {
        <div class="auth-form">
            <h2>"Вход"</h2>

            <form on:submit=on_submit>
                <div class="form-group">
                    <label for="login-email">"Email"</label>
                    <input
                        id="login-email"
                        type="email"
                        placeholder="email@example.com"
                        prop:value=email
                        on:input=move |ev| set_email.set(event_target_value(&ev))
                        prop:disabled=loading
                    />
                </div>

                <div class="form-group">
                    <label for="login-password">"Пароль"</label>
                    <input
                        id="login-password"
                        type="password"
                        placeholder="Минимум 8 символов"
                        prop:value=password
                        on:input=move |ev| set_password.set(event_target_value(&ev))
                        prop:disabled=loading
                    />
                </div>

                {move || error.get().map(|err| view! {
                    <div class="form-error">{err}</div>
                })}

                <button
                    type="submit"
                    class="auth-button"
                    prop:disabled=move || loading.get()
                >
                    {move || if loading.get() { "Входим..." } else { "Войти" }}
                </button>
            </form>

            {move || on_switch_to_register.map(|callback| view! {
                <p class="auth-switch">
                    "Нет аккаунта? "
                    <a href="#" on:click=move |ev| {
                        ev.prevent_default();
                        callback.run(());
                    }>"Зарегистрироваться"</a>
                </p>
            })}
        </div>
    }
}

/// Register form component
#[component]
pub fn RegisterForm(
    #[prop(optional)] on_success: Option<Callback<()>>,
    #[prop(optional)] on_switch_to_login: Option<Callback<()>>,
) -> impl IntoView {
    let (email, set_email) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (confirm_password, set_confirm_password) = signal(String::new());
    let (error, set_error) = signal(Option::<String>::None);
    let (success, set_success) = signal(false);
    let (loading, set_loading) = signal(false);

    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();

        let email_val = email.get();
        let password_val = password.get();
        let confirm_val = confirm_password.get();

        if email_val.trim().is_empty() || password_val.is_empty() {
            set_error.set(Some("Заполните все поля".to_string()));
            return;
        }

        if password_val != confirm_val {
            set_error.set(Some("Пароли не совпадают".to_string()));
            return;
        }

        if password_val.len() < 8 {
            set_error.set(Some("Пароль должен быть минимум 8 символов".to_string()));
            return;
        }

        set_loading.set(true);
        set_error.set(None);

        leptos::task::spawn_local(async move {
            match register_user(email_val, password_val).await {
                Ok(_) => {
                    set_success.set(true);
                    if let Some(callback) = on_success {
                        callback.run(());
                    }
                }
                Err(e) => {
                    set_error.set(Some(e.to_string()));
                }
            }
            set_loading.set(false);
        });
    };

    view! {
        <div class="auth-form">
            <h2>"Регистрация"</h2>

            {move || if success.get() {
                view! {
                    <div class="form-success">
                        "Регистрация успешна! Теперь вы можете войти."
                    </div>
                }.into_any()
            } else {
                view! {
                    <form on:submit=on_submit>
                        <div class="form-group">
                            <label for="reg-email">"Email"</label>
                            <input
                                id="reg-email"
                                type="email"
                                placeholder="email@example.com"
                                prop:value=email
                                on:input=move |ev| set_email.set(event_target_value(&ev))
                                prop:disabled=loading
                            />
                        </div>

                        <div class="form-group">
                            <label for="reg-password">"Пароль"</label>
                            <input
                                id="reg-password"
                                type="password"
                                placeholder="Минимум 8 символов"
                                prop:value=password
                                on:input=move |ev| set_password.set(event_target_value(&ev))
                                prop:disabled=loading
                            />
                        </div>

                        <div class="form-group">
                            <label for="reg-confirm">"Подтвердите пароль"</label>
                            <input
                                id="reg-confirm"
                                type="password"
                                placeholder="Повторите пароль"
                                prop:value=confirm_password
                                on:input=move |ev| set_confirm_password.set(event_target_value(&ev))
                                prop:disabled=loading
                            />
                        </div>

                        {move || error.get().map(|err| view! {
                            <div class="form-error">{err}</div>
                        })}

                        <button
                            type="submit"
                            class="auth-button"
                            prop:disabled=move || loading.get()
                        >
                            {move || if loading.get() { "Регистрация..." } else { "Зарегистрироваться" }}
                        </button>
                    </form>
                }.into_any()
            }}

            {move || on_switch_to_login.map(|callback| view! {
                <p class="auth-switch">
                    "Уже есть аккаунт? "
                    <a href="#" on:click=move |ev| {
                        ev.prevent_default();
                        callback.run(());
                    }>"Войти"</a>
                </p>
            })}
        </div>
    }
}

/// Auth modal that shows login/register forms
#[component]
pub fn AuthModal(show: ReadSignal<bool>, set_show: WriteSignal<bool>) -> impl IntoView {
    let (mode, set_mode) = signal("login"); // "login" or "register"

    let close = move |_| set_show.set(false);

    let on_login_success = Callback::new(move |_| {
        set_show.set(false);
    });

    let on_register_success = Callback::new(move |_| {
        set_mode.set("login");
    });

    let switch_to_register = Callback::new(move |_| {
        set_mode.set("register");
    });

    let switch_to_login = Callback::new(move |_| {
        set_mode.set("login");
    });

    view! {
        {move || show.get().then(|| view! {
            <div class="modal-overlay" on:click=close>
                <div class="modal-content auth-modal" on:click=move |ev| ev.stop_propagation()>
                    <button class="modal-close" on:click=close>"×"</button>

                    {move || if mode.get() == "login" {
                        view! {
                            <LoginForm
                                on_success=on_login_success
                                on_switch_to_register=switch_to_register
                            />
                        }.into_any()
                    } else {
                        view! {
                            <RegisterForm
                                on_success=on_register_success
                                on_switch_to_login=switch_to_login
                            />
                        }.into_any()
                    }}
                </div>
            </div>
        })}
    }
}

/// User menu button (shows login button or user info)
#[component]
pub fn UserMenu() -> impl IntoView {
    let auth = use_auth();
    let set_auth = use_set_auth();
    let (show_modal, set_show_modal) = signal(false);

    let logout = move |_| {
        let state = AuthState::default();
        save_auth_to_storage(&state);
        set_auth.set(state);
    };

    view! {
        <div class="user-menu">
            {move || if auth.get().is_authenticated() {
                let email = auth.get().email.clone().unwrap_or_default();
                view! {
                    <div class="user-info">
                        <span class="user-email">{email}</span>
                        <button class="logout-button" on:click=logout>
                            "Выйти"
                        </button>
                    </div>
                }.into_any()
            } else {
                view! {
                    <button class="login-button" on:click=move |_| set_show_modal.set(true)>
                        "Войти"
                    </button>
                }.into_any()
            }}

            <AuthModal show=show_modal set_show=set_show_modal />
        </div>
    }
}

/// Full-page login screen
#[component]
pub fn LoginPage() -> impl IntoView {
    let auth = use_auth();
    let navigate = leptos_router::hooks::use_navigate();
    let (mode, set_mode) = signal("login"); // "login" or "register"

    // Redirect to home if already authenticated
    let nav_effect = navigate.clone();
    Effect::new(move |_| {
        if auth.get().is_authenticated() {
            nav_effect("/", Default::default());
        }
    });

    let nav_success = navigate.clone();
    let on_login_success = Callback::new(move |_| {
        nav_success("/", Default::default());
    });

    let on_register_success = Callback::new(move |_| {
        set_mode.set("login");
    });

    let switch_to_register = Callback::new(move |_| {
        set_mode.set("register");
    });

    let switch_to_login = Callback::new(move |_| {
        set_mode.set("login");
    });

    view! {
        <div class="login-page">
            <div class="login-theme-toggle">
                <crate::components::theme_toggle::ThemeToggle />
            </div>
            <div class="login-container">
                <div class="login-header">
                    <h1>"Tea Advisor"</h1>
                    <p class="login-subtitle">"AI-помощник по выбору чая"</p>
                </div>

                {move || if mode.get() == "login" {
                    view! {
                        <LoginForm
                            on_success=on_login_success
                            on_switch_to_register=switch_to_register
                        />
                    }.into_any()
                } else {
                    view! {
                        <RegisterForm
                            on_success=on_register_success
                            on_switch_to_login=switch_to_login
                        />
                    }.into_any()
                }}
            </div>
        </div>
    }
}

/// Hook for protecting routes - call this at the start of protected components
/// Returns true if authenticated, false if redirecting
pub fn use_require_auth() -> ReadSignal<bool> {
    let auth = use_auth();
    let navigate = leptos_router::hooks::use_navigate();
    let (ready, set_ready) = signal(false);

    Effect::new(move |_| {
        if !auth.get().is_authenticated() {
            navigate("/login", Default::default());
        } else {
            set_ready.set(true);
        }
    });

    ready
}
