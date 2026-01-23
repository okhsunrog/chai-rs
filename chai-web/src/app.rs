use leptos::prelude::*;
use leptos_meta::*;
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

use crate::components::auth::{AuthProvider, LoginPage};
use crate::components::home::Home;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/chai-web.css"/>
        <Title text="Tea Advisor - AI помощник по выбору чая"/>
        <Meta name="description" content="Умный помощник для подбора чая с AI"/>

        <AuthProvider>
            <Router>
                <main>
                    <Routes fallback=|| "Page not found.">
                        <Route path=path!("/") view=Home/>
                        <Route path=path!("/login") view=LoginPage/>
                    </Routes>
                </main>
            </Router>
        </AuthProvider>
    }
}
