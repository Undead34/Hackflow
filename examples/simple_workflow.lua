---@diagnostic disable undefined-global

local function main(args)
    local target = args[3] or "example.com"
    
    flow:print("Starting simple workflow for: " .. target)
    
    -- Task 1: Run a simple command
    local cmd_handle = flow:run_command("echo", {"Scanning " .. target}, nil)
    
    -- Task 2: DNS lookup (independent task)
    local dns_handle = flow:dns_lookup({
        domain = target
    })
    
    -- Task 3: Run Subfinder (depends on DNS lookup)
    local subfinder_handle = flow:run_subfinder({
        domain = target,
        input_handle = dns_handle -- Create dependency on DNS lookup
    })
    
    -- Task 4: Process results (depends on Subfinder)
    local process_handle = flow:run_command("echo", {"Processing results"}, subfinder_handle)
    
    -- Execute all tasks in parallel, respecting dependencies
    -- The DAG will ensure proper execution order:
    -- 1. Task 1 and Task 2 can run in parallel (no dependencies)
    -- 2. Task 3 will wait for Task 2 to complete
    -- 3. Task 4 will wait for Task 3 to complete
    flow:print("Executing workflow with dependencies...")
    local success = flow:execute_parallel()
    
    if success then
        flow:print("Workflow completed successfully!")
        
        -- Retrieve and display results
        local subfinder_results = flow:get_output(subfinder_handle)
        if subfinder_results then
            flow:print("Discovered subdomains:")
            for i, subdomain in ipairs(subfinder_results) do
                flow:print("  - " .. subdomain)
            end
        end
    else
        flow:print("Workflow failed!")
    end

    success = success and flow:execute()
    
    return success
end

return { main = main }
