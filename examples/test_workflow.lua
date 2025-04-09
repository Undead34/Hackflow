---@diagnostic disable undefined-global

local function main(args)
    flow:print("Hello World!")
    flow:execute()

    return true
end

return { main = main }
