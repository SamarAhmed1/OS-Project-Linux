use crate::proc_enhanced::EnhancedProcessMetrics;

#[derive(Debug)]
pub enum FilterCondition {
    NameContains(String),
    NameExact(String),
    User(String),
    Pid(u32),
    ParentPid(u32),
    State(String),
    MinCpu(f64),
    MaxCpu(f64),
    MinMemory(u64),
    MaxMemory(u64),
}

pub struct ProcessFilter {
    conditions: Vec<FilterCondition>,
}

impl ProcessFilter {
    pub fn new() -> Self {
        ProcessFilter {
            conditions: Vec::new(),
        }
    }

    pub fn add_condition(&mut self, condition: FilterCondition) {
        self.conditions.push(condition);
    }

    pub fn matches(&self, process: &EnhancedProcessMetrics) -> bool {
        for condition in &self.conditions {
            if !self.matches_condition(process, condition) {
                return false;
            }
        }
        true
    }

    fn matches_condition(&self, process: &EnhancedProcessMetrics, condition: &FilterCondition) -> bool {
        match condition {
            FilterCondition::NameContains(pattern) => 
                process.comm.to_lowercase().contains(&pattern.to_lowercase()),
            FilterCondition::NameExact(name) => 
                process.comm == *name,
            FilterCondition::User(username) => 
                process.user == *username,
            FilterCondition::Pid(pid) => 
                process.pid == *pid,
            FilterCondition::ParentPid(ppid) => 
                process.ppid == *ppid,
            FilterCondition::State(state) => 
                process.state == *state,
            FilterCondition::MinCpu(min) => 
                process.cpu_time >= *min,
            FilterCondition::MaxCpu(max) => 
                process.cpu_time <= *max,
            FilterCondition::MinMemory(min) => 
                process.mem_usage >= *min,
            FilterCondition::MaxMemory(max) => 
                process.mem_usage <= *max,
        }
    }

    pub fn filter_processes(&self, processes: Vec<EnhancedProcessMetrics>) -> Vec<EnhancedProcessMetrics> {
        processes.into_iter()
            .filter(|p| self.matches(p))
            .collect()
    }
}