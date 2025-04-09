---@diagnostic disable undefined-global

-- Reconnaissance workflow example
-- This workflow demonstrates the DAG-based workflow system with task dependencies

local function main(args)
    -- Get the target domain from arguments or use a default
    local target_domain = args[1] or "example.com"
    
    flow:print("Starting reconnaissance workflow for: " .. target_domain)
    
    -- Step 1: Run subfinder to discover subdomains
    local subfinder_handle = flow:run_subfinder({
        domain = target_domain,
        options = {"-silent"}
    })
    
    -- Step 2: Run DNS lookups on discovered subdomains (depends on subfinder)
    local dns_handle = flow:dns_lookup({
        domain = target_domain,
        input_handle = subfinder_handle -- This creates a dependency
    })
    
    -- Step 3: Run Nmap scan on the target domain (independent task)
    local nmap_handle = flow:run_nmap({
        targets = {target_domain},
        options = {"-sV", "-p", "80,443,8080"}
    })
    
    -- Step 4: Run Wappalyzer on the target domain (independent task)
    local wappalyzer_handle = flow:run_wappalyzer({
        url = "https://" .. target_domain
    })
    
    -- Execute all tasks in parallel, respecting dependencies
    flow:print("Executing workflow in parallel...")
    local success = flow:execute_parallel()
    
    if success then
        flow:print("Workflow completed successfully!")
        
        -- Retrieve and process results
        local subfinder_results = flow:get_output(subfinder_handle)
        if subfinder_results then
            flow:print("Discovered subdomains:")
            for i, subdomain in ipairs(subfinder_results) do
                flow:print("  - " .. subdomain)
            end
        end
        
        local nmap_results = flow:get_output(nmap_handle)
        if nmap_results and nmap_results.hosts then
            flow:print("Nmap scan results:")
            for i, host in ipairs(nmap_results.hosts) do
                flow:print("  - " .. host)
            end
        end
        
        local wappalyzer_results = flow:get_output(wappalyzer_handle)
        if wappalyzer_results and wappalyzer_results.technologies then
            flow:print("Technologies detected:")
            for i, tech in ipairs(wappalyzer_results.technologies) do
                flow:print("  - " .. tech)
            end
        end
    else
        flow:print("Workflow failed!")
    end
    
    return success
end

-- Alternative approach: Sequential execution
local function sequential_example(target_domain)
    flow:print("Starting sequential workflow for: " .. target_domain)
    
    -- Define tasks
    local cmd_handle = flow:run_command("echo", {"Hello from command line"}, nil)
    local dns_handle = flow:dns_lookup({domain = target_domain})
    
    -- Execute tasks sequentially
    flow:execute()
    
    -- Process results
    local dns_results = flow:get_output(dns_handle)
    if dns_results then
        flow:print("DNS results:")
        for i, result in ipairs(dns_results) do
            flow:print("  - " .. result)
        end
    end
    
    return true
end

-- Chain tasks example
local function chain_tasks_example(target_domain)
    -- Step 1: Run subfinder
    local subfinder_handle = flow:run_subfinder({domain = target_domain})
    
    -- Step 2: Run nmap on subfinder results
    local nmap_handle = flow:run_nmap({
        targets = {target_domain}, -- This will be overridden by input from subfinder
        input_handle = subfinder_handle
    })
    
    -- Step 3: Run wappalyzer on each host from nmap results
    local wappalyzer_handle = flow:run_wappalyzer({
        url = "https://" .. target_domain, -- This will be overridden by input from nmap
        input_handle = nmap_handle
    })
    
    -- Execute the workflow
    flow:execute_parallel()
    
    return true
end

return { 
    main = main,
    sequential_example = sequential_example,
    chain_tasks_example = chain_tasks_example
}
