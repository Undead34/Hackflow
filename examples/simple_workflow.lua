---@diagnostic disable undefined-global

local function main(args)
    local h = flow:dns_lookup({ domain = "domain" })
    flow:execute()
    local result = flow:get_output(h)
    print(result[1])
end

return { main = main }
