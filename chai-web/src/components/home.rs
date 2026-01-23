use crate::components::auth::{UserMenu, use_auth, use_require_auth};
use crate::components::tea_card::TeaCard;
use crate::components::theme_toggle::ThemeToggle;
use crate::models::AIResponse;
use crate::utils::russian_plural;
use leptos::prelude::*;

#[server]
pub async fn get_tea_recommendations(
    query: String,
    token: String,
) -> Result<AIResponse, ServerFnError> {
    use crate::server::{ai::chat_completion, auth};
    use std::time::Instant;

    // Validate JWT token
    let claims = auth::validate_token(&token).map_err(|_| ServerFnError::new("Unauthorized"))?;

    let start = Instant::now();

    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| ServerFnError::new("API key not configured"))?;

    let result = chat_completion(query.clone(), api_key).await;
    let duration_ms = start.elapsed().as_millis();

    match &result {
        Ok(response) => {
            tracing::info!(
                user_id = %claims.sub,
                query = %query,
                results = response.tea_cards.len(),
                duration_ms = %duration_ms,
                "Search completed"
            );
        }
        Err(e) => {
            tracing::error!(
                user_id = %claims.sub,
                query = %query,
                error = %e,
                duration_ms = %duration_ms,
                "Search failed"
            );
        }
    }

    result.map_err(|e| ServerFnError::new(e.to_string()))
}

#[server]
pub async fn get_teas_count() -> Result<usize, ServerFnError> {
    // Public endpoint - no auth required (just shows count)
    use crate::server::qdrant;

    qdrant::count_teas()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))
}

#[component]
pub fn Home() -> impl IntoView {
    // Auth check - redirects to /login if not authenticated
    let auth_ready = use_require_auth();

    let (query, set_query) = signal(String::new());
    let (response, set_response) = signal(Option::<AIResponse>::None);
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal(Option::<String>::None);

    // Загружаем количество чаёв из БД
    let teas_count = Resource::new(|| (), |_| async { get_teas_count().await });

    // Get auth state for token
    let auth = use_auth();

    // Shared search function
    let do_search = move |search_query: String| {
        if search_query.trim().is_empty() || loading.get() {
            return;
        }

        let token = auth.get().token.clone().unwrap_or_default();

        set_loading.set(true);
        set_error.set(None);

        leptos::task::spawn_local(async move {
            match get_tea_recommendations(search_query, token).await {
                Ok(ai_response) => {
                    set_response.set(Some(ai_response));
                    set_error.set(None);
                }
                Err(e) => {
                    set_error.set(Some(format!("Ошибка: {}", e)));
                    leptos::logging::error!("API Error: {}", e);
                }
            }
            set_loading.set(false);
        });
    };

    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        do_search(query.get());
    };

    // Handle Enter key (Shift+Enter for new line)
    let on_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Enter" && !ev.shift_key() {
            ev.prevent_default();
            do_search(query.get());
        }
    };

    // Execute example query directly
    let run_example = move |text: &'static str| {
        set_query.set(text.to_string());
        do_search(text.to_string());
    };

    let reset_search = move |_| {
        set_response.set(None);
        set_error.set(None);
        set_query.set(String::new());
    };

    view! {
        <Show
            when=move || auth_ready.get()
            fallback=|| view! { <div class="loading">"Загрузка..."</div> }
        >
        <div class="home-container">
            <div class="top-bar">
                <UserMenu />
                <ThemeToggle />
            </div>
            <header class="hero">
                {move || if response.get().is_some() {
                    // Когда есть результаты - показываем кликабельный заголовок
                    view! {
                        <div class="hero-clickable" on:click=reset_search>
                            <h1>"🍵 Tea Advisor"</h1>
                            <p class="back-hint">"(нажмите, чтобы начать новый поиск)"</p>
                        </div>
                    }.into_any()
                } else {
                    // Когда нет результатов - обычный заголовок
                    view! {
                        <>
                            <h1>"🍵 Tea Advisor"</h1>
                            <p class="tagline">"AI-помощник для подбора идеального чая"</p>
                            <p class="subtitle">
                                <Suspense fallback=move || "Опишите что вы хотите, и я найду для вас лучшие чаи">
                                    {move || {
                                        teas_count.get().map(|result| match result {
                                            Ok(count) => format!("Опишите что вы хотите, и я найду для вас лучшие чаи из {} вариантов", count),
                                            Err(_) => "Опишите что вы хотите, и я найду для вас лучшие чаи".to_string()
                                        })
                                    }}
                                </Suspense>
                            </p>
                        </>
                    }.into_any()
                }}
            </header>

            <form class="search-form" on:submit=on_submit>
                <div class="search-input-container">
                    <textarea
                        class="search-input"
                        placeholder="Опишите что вы хотите... (Enter для отправки, Shift+Enter для новой строки)"
                        rows="3"
                        prop:value=query
                        on:input=move |ev| set_query.set(event_target_value(&ev))
                        on:keydown=on_keydown
                        prop:disabled=loading
                    />
                </div>

                <button
                    type="submit"
                    class="search-button"
                    prop:disabled=move || loading.get() || query.get().trim().is_empty()
                >
                    {move || if loading.get() {
                        "🔍 Ищу идеальный чай..."
                    } else {
                        "Найти чай"
                    }}
                </button>
            </form>

            // Кликабельные примеры запросов
            {move || if response.get().is_none() && !loading.get() {
                Some(view! {
                    <section class="examples">
                        <h3>"💡 Примеры запросов:"</h3>
                        <div class="example-queries">
                            <ExampleQuery text="Согревающий пряный чай для холодного вечера" on_click=run_example/>
                            <ExampleQuery text="Один необычный чай с дымными нотками, не набор" on_click=run_example/>
                            <ExampleQuery text="Несколько ягодных чаёв с кислинкой, только в наличии" on_click=run_example/>
                            <ExampleQuery text="Цветочный чай без ромашки" on_click=run_example/>
                            <ExampleQuery text="Много разных вариантов с бергамотом" on_click=run_example/>
                            <ExampleQuery text="Пару чаёв для бодрости утром" on_click=run_example/>
                        </div>
                    </section>
                })
            } else {
                None
            }}

            // Ошибки
            {move || error.get().map(|err| view! {
                <div class="error-message">
                    <span class="icon">"⚠️"</span>
                    <span>{err}</span>
                </div>
            })}

            // Результаты
            {move || response.get().map(|r| {
                let answer = r.answer.clone();
                let cards = r.tea_cards.clone();
                let cards_count = cards.len();
                let cards_for_each = cards.clone();

                view! {
                    <div class="results-container">
                        // Текстовый ответ AI
                        <div class="ai-answer">
                            <p class="answer-text">{answer}</p>
                        </div>

                        // Карточки чаёв
                        <div class="tea-cards-section">
                            <h2 class="cards-title">
                                "Найдено " {cards_count} " "
                                {russian_plural(cards_count, "чай", "чая", "чаёв")}
                            </h2>

                            <div class="tea-cards-grid">
                                <For
                                    each=move || cards_for_each.clone()
                                    key=|card| card.url.clone()
                                    children=move |card| view! {
                                        <TeaCard card=card />
                                    }
                                />
                            </div>
                        </div>
                    </div>
                }
            })}
        </div>
        </Show>
    }
}

#[component]
fn ExampleQuery(
    text: &'static str,
    on_click: impl Fn(&'static str) + Copy + 'static,
) -> impl IntoView {
    view! {
        <button
            class="example-query"
            on:click=move |_| on_click(text)
        >
            <span class="icon">"💭"</span>
            <span class="text">{text}</span>
        </button>
    }
}
