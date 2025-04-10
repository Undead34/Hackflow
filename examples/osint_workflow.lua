---@diagnostic disable undefined-global

-- Función para convertir una cadena hexadecimal a bytes
local function hex_to_string(hex)
    return (hex:gsub('..', function(cc)
        return string.char(tonumber(cc, 16))
    end))
end

-- Función para mostrar la salida con colores
local function print_with_colors(hex_str)
    local raw_str = hex_to_string(hex_str)
    print(raw_str)
end

local function main(args)
    -- flow:create_dir("documentacion")
    flow:create_dir("reconocimiento/pasivo")
    -- flow:create_dir("reconocimiento/activo")
    -- flow:create_dir("escaneo/automatizado")
    -- flow:create_dir("escaneo/manual")
    -- flow:create_dir("explotacion/exploits_utilizados")
    -- flow:create_dir("explotacion/payloads")
    -- flow:create_dir("explotacion/scripts_personalizados")
    -- flow:create_dir("explotacion/pruebas_de_concepto")
    -- flow:create_dir("post_explotacion")
    -- flow:create_dir("informes")
    -- flow:create_dir("herramientas")

    flow:execute()

    -- Ejecutamos el DNS lookup
    local dns_handler = flow:dns_lookup({ domain = "sitca-ve.com" })

    -- Verificar el estado detallado de la tarea
    local task_status = flow:check_task_status(dns_handler)

    if not task_status.success then
        flow:print("Error en la tarea DNS lookup:")
        if task_status.exit_code then
            flow:print("  Código de salida: " .. task_status.exit_code)
        end
        if task_status.error then
            flow:print("  Error: " .. task_status.error)
        end
        if task_status.stderr and task_status.stderr ~= "" then
            flow:print("  Stderr: " .. task_status.stderr)
        end
        flow:execute()
    else
        flow:pretty(dns_handler)
        flow:export_json({ handle = dns_handler, dir_path = "reconocimiento/pasivo" })
        flow:export_csv({ handle = dns_handler, dir_path = "reconocimiento/pasivo" })
        flow:export_raw({ handle = dns_handler, dir_path = "reconocimiento/pasivo" })
    end

    local status = flow:execute_parallel()
end

return { main = main }
