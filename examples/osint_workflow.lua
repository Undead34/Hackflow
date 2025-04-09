---@diagnostic disable undefined-global

local function main(args)
    flow:create_dir("documentacion")
    flow:create_dir("reconocimiento/pasivo")
    flow:create_dir("reconocimiento/activo")
    flow:create_dir("escaneo/automatizado")
    flow:create_dir("escaneo/manual")
    flow:create_dir("explotacion/exploits_utilizados")
    flow:create_dir("explotacion/payloads")
    flow:create_dir("explotacion/scripts_personalizados")
    flow:create_dir("explotacion/pruebas_de_concepto")
    flow:create_dir("post_explotacion")
    flow:create_dir("informes")
    flow:create_dir("herramientas")

    flow:execute_parallel()

    -- Ejecutamos el DNS lookup
    local dns_handler = flow:dns_lookup({ domain = "sitca-ve.com" })
    flow:execute_parallel()

    flow:pretty(dns_handler)
    flow:export_json({ handle = dns_handler, dir_path = "reconocimiento/pasivo" })
    flow:export_csv({ handle = dns_handler, dir_path = "reconocimiento/pasivo" })
end

return { main = main }
