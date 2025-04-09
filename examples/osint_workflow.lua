---@diagnostic disable undefined-global

local function main(args)
    -- flow:create_dir("documentacion")
    -- flow:create_dir("reconocimiento/pasivo")
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

    local dns_handler = flow:dns_lookup({ domain = "sitca-ve.com" })
    flow:export_json(dns_handler, "reconocimiento/pasivo")
    flow:execute()
end

return { main = main }
