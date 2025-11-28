use crate::filter::{ProcessFilter, FilterCondition};
use crate::proc_enhanced::{get_all_enhanced_processes, EnhancedProcessMetrics};

pub struct SearchService;

impl SearchService {
    pub fn new() -> Self {
        SearchService
    }

    pub fn search_processes(
        &self,
        name: &str,
        exact: bool,
        user: Option<&str>,
        state: Option<&str>,
        min_cpu: Option<f64>,
        max_cpu: Option<f64>,
        min_memory: Option<u64>,
        max_memory: Option<u64>,
    ) -> Result<Vec<EnhancedProcessMetrics>, String> {
        
        let mut filter = ProcessFilter::new();
        
        if exact {
            filter.add_condition(FilterCondition::NameExact(name.to_string()));
        } else {
            filter.add_condition(FilterCondition::NameContains(name.to_string()));
        }
        
        if let Some(user_filter) = user {
            filter.add_condition(FilterCondition::User(user_filter.to_string()));
        }
        
        if let Some(state_filter) = state {
            filter.add_condition(FilterCondition::State(state_filter.to_string()));
        }
        
        if let Some(min) = min_cpu {
            filter.add_condition(FilterCondition::MinCpu(min));
        }
        
        if let Some(max) = max_cpu {
            filter.add_condition(FilterCondition::MaxCpu(max));
        }
        
        if let Some(min) = min_memory {
            filter.add_condition(FilterCondition::MinMemory(min));
        }
        
        if let Some(max) = max_memory {
            filter.add_condition(FilterCondition::MaxMemory(max));
        }
        
        match get_all_enhanced_processes() {
            Ok(processes) => Ok(filter.filter_processes(processes)),
            Err(e) => Err(format!("Failed to read processes: {}", e)),
        }
    }

    pub fn display_results(&self, processes: &[EnhancedProcessMetrics]) {
        println!("Found {} matching processes:", processes.len());
        println!("{:<8} {:<20} {:<12} {:<6} {:<8} {:<12} {:<12}", 
                 "PID", "Process", "User", "State", "%CPU", "Memory(KB)", "Parent PID");
        println!("{}", "-".repeat(85));
        
        for process in processes {
            println!("{:<8} {:<20} {:<12} {:<6} {:<8.2} {:<12} {:<12}", 
                     process.pid, 
                     process.comm, 
                     process.user, 
                     process.state, 
                     process.cpu_time, 
                     process.mem_usage, 
                     process.ppid);
        }
    }
}