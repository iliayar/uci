use gloo_storage::Storage;
use leptos::*;
use leptos_router::{use_params_map, Outlet, A};

use super::auth::Auth;
use super::projects::Projects;
use super::views::ReloadConfig;

fn get_dark_mode_preferred() -> Option<bool> {
    let matches = web_sys::window()
        .unwrap()
        .match_media("(prefers-color-scheme: dark)");

    if let Ok(Some(matches)) = matches {
        Some(matches.matches())
    } else {
        None
    }
}

fn get_dark_mode_init() -> bool {
    gloo_storage::LocalStorage::get::<bool>("dark_mode")
        .ok()
        .or_else(|| get_dark_mode_preferred())
        .unwrap_or(false)
}

#[derive(Clone)]
pub struct RunnerUrl {
    pub url: String,
    pub ws_url: String,
}

#[derive(Clone)]
pub struct HeaderProject(pub Option<String>);

#[component]
pub fn Header(
    cx: Scope,
    dark_mode: ReadSignal<bool>,
    set_dark_mode: WriteSignal<bool>,
    set_token: WriteSignal<Option<String>>,
    header_project: ReadSignal<HeaderProject>,
) -> impl IntoView {
    let (project_selection, set_project_selection) = create_signal(cx, false);
    let toggle_project_selection = move |_| {
        set_project_selection(!project_selection());
    };

    let project_selection_text = move || match header_project().clone().0 {
        None => view! {cx, "project"}.into_view(cx),
        Some(project) => view! {cx, <code class="font-bold">{project}</code>}.into_view(cx),
    };

    let turn_on_dark_mode = move |_| {
        gloo_storage::LocalStorage::set("dark_mode", true).ok();
        set_dark_mode(true);
    };

    let turn_off_dark_mode = move |_| {
        gloo_storage::LocalStorage::set("dark_mode", false).ok();
        set_dark_mode(false);
    };

    view! {cx,
      <div
        class="w-full bg-bg-light dark:bg-bg-dark border-b-2 border-border-light dark:border-border-dark h-fit flex flex-row justify-between text-fg-light dark:text-fg-dark"
      >
        <div class="flex flex-row items-center">
          <div class="pl-1 pr-5">"uCI"</div>
          <div
            class="relative"
          >
            <button
              class="hover:bg-button-focus-light hover:dark:bg-button-focus-dark bg-bg-light dark:bg-bg-dark"
              on:click=toggle_project_selection
            >
              {project_selection_text} " "
              <i class="fa-solid fa-angle-down"></i>
            </button>
            <Show when=project_selection fallback={|_| view!{cx,}}>
              <div
                class="absolute min-w-full w-fit bg-bg-light dark:bg-bg-dark p-1 text-center border-b-2 border-x-2 border-border-light dark:border-border-dark"
              >
                <Projects />
              </div>
            </Show>
            <div class="inline ml-2">
              <ReloadConfig />
            </div>
          </div>
        </div>
        <div class="flex flex-row justify-between">
          <Auth set_token />
          <button class="hover:bg-button-focus-light hover:dark:bg-button-focus-dark">
            <Show
              when=dark_mode
              fallback={move |_| view!{cx, <i class="fa-regular fa-moon p-1" on:click=turn_on_dark_mode></i>}}
            >
              <i class="fa-regular fa-sun p-1" on:click=turn_off_dark_mode></i>
            </Show>
          </button>
        </div>
      </div>
    }
}

#[component]
pub fn Body(cx: Scope) -> impl IntoView {
    let runner_url: RunnerUrl = expect_context(cx);

    let (dark_mode, set_dark_mode) = create_signal(cx, get_dark_mode_init());

    let init_token =
        gloo_storage::LocalStorage::get::<Option<String>>("uci_token").unwrap_or_else(|_| None);
    let (token, set_token) = create_signal::<Option<String>>(cx, init_token.clone());

    let config = Signal::derive(cx, move || crate::config::Config {
        runner_url: runner_url.url.clone(),
        ws_runner_url: runner_url.ws_url.clone(),
        token: token(),
    });

    create_effect(cx, move |_| {
        let token = token();

        gloo_storage::LocalStorage::set("uci_token", token.clone()).ok();
    });

    provide_context(cx, config);

    let (header_project, set_header_project) = create_signal(cx, HeaderProject(None));
    provide_context(cx, set_header_project);

    view! {cx,
      <div class="h-screen flex flex-col" class:dark=dark_mode>
      <Header
        dark_mode=dark_mode
        set_dark_mode=set_dark_mode
        set_token=set_token
        header_project=header_project
      />
        <Outlet />
      </div>
    }
}

#[component]
pub fn BodyWithoutProject(cx: Scope) -> impl IntoView {
    let set_header_project: WriteSignal<HeaderProject> = expect_context(cx);
    set_header_project(HeaderProject(None));

    view! {cx,
      <div class="grow flex flex-row w-full text-fg-light dark:text-fg-dark bg-bg-light dark:bg-bg-dark">
    "No project selected"
      </div>
    }
}

#[component]
pub fn BodyWithProject(cx: Scope) -> impl IntoView {
    let set_header_project: WriteSignal<HeaderProject> = expect_context(cx);

    let params = use_params_map(cx);
    let project = Signal::derive(cx, move || {
        params.with(|params| params.get("project").cloned().map(crate::types::ProjectId))
    });

    provide_context(cx, project);

    create_effect(cx, move |_| {
        set_header_project(HeaderProject(Some(project().unwrap().0)));
    });

    view! {cx,
      <div class="grow flex flex-row w-full text-fg-light dark:text-fg-dark bg-bg-light dark:bg-bg-dark">
        <MainNavigation/>
        <div class="p-4 w-full"><Outlet/></div>
      </div>
    }
}

#[component]
fn ProjectSection(cx: Scope, href: &'static str, title: &'static str) -> impl IntoView {
    view! {cx,
      <A
        class="p-0.5 w-full text-left hover:bg-button-focus-light hover:dark:bg-button-focus-dark block"
        active_class="bg-section-active-light dark:bg-section-active-dark"
        href={href}
      >
        {title}
      </A>
    }
}

#[component]
fn MainNavigation(cx: Scope) -> impl IntoView {
    view! {cx,
      <div class="border-r-2 border-border-light dark:border-border-dark basis-1/5">
        <div class="pt-1">
          <ProjectSection href="overview" title="Overview" />
          <ProjectSection href="actions" title="Actions" />
          <ProjectSection href="runs" title="Runs" />
        </div>
      </div>
    }
}
