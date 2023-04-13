use std::collections::{HashSet, HashMap};

#[derive(Debug)]
pub struct MatchedActions {
    pub reload_config: bool,
    pub run_pipelines: HashMap<String, HashSet<String>>,
    pub services: HashMap<String, HashMap<String, super::ServiceAction>>,
    pub reload_projects: HashSet<String>,
}

impl Default for MatchedActions {
    fn default() -> Self {
        Self {
            reload_config: false,
            run_pipelines: Default::default(),
            services: Default::default(),
            reload_projects: Default::default(),
        }
    }
}

impl MatchedActions {
    pub fn is_empty(&self) -> bool {
        !self.reload_config
            && self.reload_projects.is_empty()
            && self.run_pipelines.is_empty()
            && self.services.is_empty()
    }

    pub fn add_project(
        &mut self,
        project_id: &str,
        super::ProjectMatchedActions {
            reload_config,
            run_pipelines,
            services,
            reload_project,
        }: super::ProjectMatchedActions,
    ) {
        self.reload_config |= reload_config;
        if !run_pipelines.is_empty() {
            self.run_pipelines
                .insert(project_id.to_string(), run_pipelines);
        }
        if !services.is_empty() {
            self.services.insert(project_id.to_string(), services);
        }
        if reload_project {
            self.reload_projects.insert(project_id.to_string());
        }
    }

    pub fn get_project(&self, project_id: &str) -> Option<super::ProjectMatchedActions> {
        let run_pipelines = self
            .run_pipelines
            .get(project_id)
            .cloned()
            .unwrap_or_default();
        let services = self.services.get(project_id).cloned().unwrap_or_default();
        let reload_project = self.reload_projects.contains(project_id);

        let res = super::ProjectMatchedActions {
            reload_config: false,
            run_pipelines,
            services,
            reload_project,
        };

        if res.is_empty() {
            None
        } else {
            Some(res)
        }
    }

    pub fn merge(&mut self, other: MatchedActions) {
        self.reload_config |= other.reload_config;

        for project_id in other.reload_projects.into_iter() {
            self.reload_projects.insert(project_id);
        }

        for (project_id, run_pipelines) in other.run_pipelines.into_iter() {
            if self.run_pipelines.contains_key(&project_id) {
                self.run_pipelines
                    .insert(project_id.clone(), HashSet::new());
            }

            let cur_run_pipelines = self.run_pipelines.get_mut(&project_id).unwrap();
            for pipeline_id in run_pipelines.into_iter() {
                cur_run_pipelines.insert(pipeline_id);
            }
        }

        for (project_id, services) in other.services.into_iter() {
            if self.services.contains_key(&project_id) {
                self.services.insert(project_id.clone(), HashMap::new());
            }
            let cur_services = self.services.get_mut(&project_id).unwrap();
            for (service, action) in services.into_iter() {
                cur_services.insert(service, action);
            }
        }
    }
}
