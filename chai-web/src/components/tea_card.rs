use crate::models::TeaCard as TeaCardModel;
use leptos::prelude::*;

#[component]
pub fn TeaCard(card: TeaCardModel) -> impl IntoView {
    let (show_modal, set_show_modal) = signal(false);

    let open_modal = move |_| {
        set_show_modal.set(true);
    };

    let close_modal = move |_| {
        set_show_modal.set(false);
    };

    let match_percentage = (card.match_score.clamp(0.0, 1.0) * 100.0) as u32;

    // Определяем цвет индикатора совпадения
    let match_color = if match_percentage >= 80 {
        "high"
    } else if match_percentage >= 60 {
        "medium"
    } else {
        "low"
    };

    // Клонируем данные для использования в замыканиях
    let title = card.title.clone();
    let image_url = card.image_url.clone();
    let in_stock = card.in_stock;
    let tags = card.tags.clone();
    let short_description = card.short_description.clone();
    let price = card.price.clone();
    let composition = card.composition.clone();
    let url = card.url.clone();
    let sample_url = card.sample_url.clone();
    let sample_in_stock = card.sample_in_stock;
    let description = card.description.clone();
    let series = card.series.clone();
    let full_composition = card.full_composition.clone();
    let price_variants = card.price_variants.clone();

    // Определяем статус наличия
    let availability_status = if !in_stock && sample_in_stock {
        "sample_only" // В наличии только пробник
    } else if in_stock {
        "available" // В наличии
    } else {
        "unavailable" // Нет в наличии
    };

    view! {
        <>
            // Карточка (кликабельная)
            <div
                class="tea-card"
                role="button"
                tabindex="0"
                on:click=open_modal
                on:keydown=move |e: web_sys::KeyboardEvent| {
                    if e.key() == "Enter" || e.key() == " " {
                        e.prevent_default();
                        set_show_modal.set(true);
                    }
                }
            >
                // Изображение
                {image_url.clone().map(|url| view! {
                    <div class="card-image">
                        <img src=url.clone() alt=title.clone() loading="lazy"/>
                        {if !in_stock {
                            Some(view! {
                                <div class="out-of-stock-badge">"Нет в наличии"</div>
                            })
                        } else {
                            None
                        }}
                    </div>
                })}

                <div class="card-content">
                // Заголовок и совпадение
                <div class="card-header">
                    <h3 class="card-title">{title.clone()}</h3>
                    <div class=format!("match-badge match-{}", match_color)>
                        {format!("{}% совпадение", match_percentage)}
                    </div>
                </div>

                // Теги
                {if !tags.is_empty() {
                    let tags_clone = tags.clone();
                    Some(view! {
                        <div class="card-tags">
                            <For
                                each=move || tags_clone.clone()
                                key=|tag| tag.clone()
                                children=move |tag: String| {
                                    let tag_text = tag.clone();
                                    view! {
                                        <span class="tag">{tag_text}</span>
                                    }
                                }
                            />
                        </div>
                    })
                } else {
                    None
                }}

                // Краткое описание
                <p class="recommendation-reason">
                    <span class="icon">"✨"</span>
                    {short_description.clone()}
                </p>

                // Цена
                {price.clone().map(|p| {
                    let price_text = p.clone();
                    view! {
                        <div class="card-price">
                            <span class="price-label">"Цена: "</span>
                            <span class="price-value">{price_text}</span>
                        </div>
                    }
                })}
                </div>
            </div>

            // Модальное окно
            {move || if show_modal.get() {
                let status = availability_status;
                Some(view! {
                    <div
                        class="modal-overlay"
                        role="dialog"
                        aria-modal="true"
                        aria-labelledby="modal-title"
                        on:click=close_modal
                        on:keydown=move |e: web_sys::KeyboardEvent| {
                            if e.key() == "Escape" {
                                set_show_modal.set(false);
                            }
                        }
                    >
                        <div class="modal-content" on:click=move |e: web_sys::MouseEvent| e.stop_propagation()>
                            <button class="modal-close" aria-label="Закрыть" on:click=close_modal>"✕"</button>

                            <div class="modal-body">
                                // Изображение с бейджем статуса
                                {image_url.clone().map(|url| view! {
                                    <div class="modal-image-wrapper">
                                        <div class="modal-image">
                                            <img src=url alt=title.clone() loading="lazy"/>
                                        </div>
                                        // Статус наличия как overlay
                                        <div class=move || format!("availability-badge badge-{}", status)>
                                            {match status {
                                                "available" => "✅ В наличии",
                                                "sample_only" => "🔬 Только пробник",
                                                _ => "❌ Нет в наличии"
                                            }}
                                        </div>
                                    </div>
                                })}

                                // Заголовок и метаинфо
                                <div class="modal-header">
                                    <h2 id="modal-title" class="modal-title">{title.clone()}</h2>
                                    <div class=format!("match-badge match-{}", match_color)>
                                        {format!("{}% совпадение", match_percentage)}
                                    </div>
                                </div>

                                // Серия
                                {series.clone().map(|s| view! {
                                    <div class="modal-series">
                                        <span class="series-icon">"📚"</span>
                                        <span class="series-name">{s}</span>
                                    </div>
                                })}

                                // Теги
                                {if !tags.is_empty() {
                                    let tags_modal = tags.clone();
                                    Some(view! {
                                        <div class="modal-tags">
                                            <For
                                                each=move || tags_modal.clone()
                                                key=|tag| tag.clone()
                                                children=move |tag: String| view! {
                                                    <span class="tag">{tag}</span>
                                                }
                                            />
                                        </div>
                                    })
                                } else {
                                    None
                                }}

                                // Краткое описание
                                <div class="modal-section modal-recommendation">
                                    <h3>"✨ Почему рекомендуем"</h3>
                                    <p>{short_description.clone()}</p>
                                </div>

                                // Описание
                                {description.clone().map(|desc| view! {
                                    <div class="modal-section modal-description">
                                        <h3>"📖 Описание"</h3>
                                        <p>{desc}</p>
                                    </div>
                                })}

                                // Состав
                                {if !full_composition.is_empty() || !composition.is_empty() {
                                    let comp_to_show = if !full_composition.is_empty() {
                                        full_composition.clone()
                                    } else {
                                        composition.clone()
                                    };
                                    Some(view! {
                                        <div class="modal-section modal-composition">
                                            <h3>"🌿 Состав"</h3>
                                            <ul class="composition-list">
                                                <For
                                                    each=move || comp_to_show.clone()
                                                    key=|ing| ing.clone()
                                                    children=move |ingredient: String| view! {
                                                        <li>{ingredient}</li>
                                                    }
                                                />
                                            </ul>
                                        </div>
                                    })
                                } else {
                                    None
                                }}

                                // Варианты цен
                                {if !price_variants.is_empty() {
                                    let variants = price_variants.clone();
                                    Some(view! {
                                        <div class="modal-section modal-price-variants">
                                            <h3>"💰 Варианты упаковки"</h3>
                                            <div class="price-variants-list">
                                                <For
                                                    each=move || variants.clone()
                                                    key=|v| format!("{}-{}", v.packaging, v.price)
                                                    children=move |variant| {
                                                        let packaging = variant.packaging.clone();
                                                        let price_val = variant.price.clone();
                                                        let quantity = variant.quantity.clone();
                                                        view! {
                                                            <div class="price-variant-item">
                                                                <div class="variant-packaging">{packaging}</div>
                                                                <div class="variant-details">
                                                                    <span class="variant-quantity">{quantity}</span>
                                                                    <span class="variant-price">{price_val}</span>
                                                                </div>
                                                            </div>
                                                        }
                                                    }
                                                />
                                            </div>
                                        </div>
                                    }.into_any())
                                } else { price.clone().map(|p| view! {
                                        <div class="modal-section modal-simple-price">
                                            <h3>"💰 Цена"</h3>
                                            <div class="simple-price">{p}</div>
                                        </div>
                                    }.into_any()) }}

                                // Кнопки действий (ВСЕГДА видны)
                                <div class="modal-actions">
                                    // Основная кнопка (магазин)
                                    <a
                                        href=url.clone()
                                        target="_blank"
                                        rel="noopener noreferrer"
                                        class=move || {
                                            if in_stock {
                                                "btn btn-primary"
                                            } else {
                                                "btn btn-disabled"
                                            }
                                        }
                                    >
                                        {move || if in_stock {
                                            "🛒 Купить в магазине"
                                        } else {
                                            "❌ Нет в наличии"
                                        }}
                                    </a>

                                    // Кнопка пробника (если есть)
                                    {sample_url.clone().map(|s_url| view! {
                                        <a
                                            href=s_url
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            class=move || {
                                                if sample_in_stock {
                                                    "btn btn-sample"
                                                } else {
                                                    "btn btn-sample-disabled"
                                                }
                                            }
                                        >
                                            {move || if sample_in_stock {
                                                "🔬 Купить пробник"
                                            } else {
                                                "🔬 Пробник недоступен"
                                            }}
                                        </a>
                                    })}
                                </div>
                            </div>
                        </div>
                    </div>
                })
            } else {
                None
            }}
        </>
    }
}
