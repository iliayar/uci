use leptos::*;
use leptos_router::*;

use crate::views;
use crate::body;

#[component]
fn NotFound(_cx: Scope) -> impl IntoView {
    view!{cx, "Not found"}
}

#[component]
pub fn UiRoutes(cx: Scope) -> impl IntoView {
    view! {cx,
      <Router>
        <Routes>
          <Route
            path="/"
            view=body::Body
          >
            <Route
              path="/projects/:project"
              view=body::BodyWithProject
            >
             <Route
               path="runs"
               view=views::Runs
             >
               <Route
                 path=":run/:pipeline"
                 view=views::Run
               >
               </Route>
               <Route
                 path="/"
                 view=views::NoRunSelected
               >
               </Route>
             </Route>
  
             <Route
               path="actions"
               view=views::Actions
             >
             </Route>

             <Route
               path="overview"
               view=views::Overview
             >
             </Route>
             <Route
               path="/"
               view=views::Overview
             >
             </Route>
            </Route>
            <Route
              path="/"
              view=body::BodyWithoutProject
            >
            </Route>
            <Route
              path="/*any"
              view=NotFound
            >
            </Route>
	  </Route>
        </Routes>
      </Router>
    }
}
