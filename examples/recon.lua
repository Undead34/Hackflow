-- recon.lua
-- Un flujo de trabajo de reconocimiento completo usando Hackflow.
-- Uso: hackflow --file recon.lua --args 'domain="example.com", intensive=true'

local function main(args)
    if not args.domain then
        print("❌ Error: Se requiere un dominio. --args 'domain=\"example.com\"'")
        return
    end

    local target = args.domain
    local results_dir = "reconocimiento/" .. target
    local wordlist = "/usr/share/wordlists/dirb/common.txt" -- Cambiar según tu sistema

    ---------------------------------------------------------------------
    -- FASE 1: PREPARACIÓN (Comportamiento Síncrono)
    -- Creamos y ejecutamos un flujo pequeño para asegurar que la
    -- estructura de directorios exista antes de continuar.
    ---------------------------------------------------------------------
    print("🚀 FASE 1: Creando estructura de directorios...")
    local setup_flow = Flow.new("Preparacion")
    local dirs = { "subdominios", "hosts_vivos", "escaneos", "metadata" }
    for _, dir in ipairs(dirs) do
        setup_flow:add_task("crear_dir_" .. dir)
            :command("mkdir -p " .. results_dir .. "/" .. dir)
    end
    local setup_report = setup_flow:run()
    if not setup_report.success then
        print("❌ Error fatal al crear directorios. Abortando.")
        return
    end
    print("✅ Directorios listos.")

    ---------------------------------------------------------------------
    -- FASES 2, 3 y 4: RECONOCIMIENTO (Paralelo, Dependiente y Condicional)
    -- Este es el flujo principal. Lo definimos TODO de una vez para
    -- que el motor lo optimice al máximo, y lo ejecutamos al final.
    ---------------------------------------------------------------------
    print("🚀 FASE 2, 3 y 4: Iniciando reconocimiento masivo...")
    local recon_flow = Flow.new("Reconocimiento Masivo")

    -- Tareas en Paralelo (FASE 2)
    local assetfinder = recon_flow:add_task("assetfinder")
        :command("assetfinder --subs-only " .. target .. " > " .. results_dir .. "/subdominios/assetfinder.txt")

    local amass = recon_flow:add_task("amass")
        :command("amass enum -d " .. target .. " -o " .. results_dir .. "/subdominios/amass.txt")

    -- Tareas Dependientes (FASE 3)
    local consolidate = recon_flow:add_task("consolidar_subdominios")
        :command("cat " .. results_dir .. "/subdominios/*.txt | sort -u > " .. results_dir .. "/subdominios_unicos.txt")
        :depends_on({ assetfinder, amass })

    local httpx = recon_flow:add_task("encontrar_hosts_vivos")
        :command("cat " .. results_dir .. "/subdominios_unicos.txt | httpx -silent -o " .. results_dir .. "/hosts_vivos.txt")
        :depends_on(consolidate)

    -- Tarea Condicional (FASE 4)
    local nuclei = recon_flow:add_task("escanear_con_nuclei")
        :command("nuclei -l " .. results_dir .. "/hosts_vivos.txt -o " .. results_dir .. "/escaneos/nuclei.txt")
        :depends_on(httpx)
        :run_if(function()
            return args.intensive == true
        end)

    -- La gran ejecución: El motor optimiza y ejecuta todo el plan
    local recon_report = recon_flow:run()
    if not recon_report.success then
        print("❌ Error durante la fase de reconocimiento: " .. recon_report.failed_task)
    end
    print("✅ Reconocimiento masivo completado.")

    ---------------------------------------------------------------------
    -- FASE 5: REPORTE (Comportamiento Síncrono Final)
    -- Un último flujo simple para empaquetar los resultados.
    ---------------------------------------------------------------------
    print("🚀 FASE 5: Generando reporte final...")
    local report_flow = Flow.new("Empaquetado")
    report_flow:add_task("comprimir_resultados")
        :command("tar -czvf " .. target .. "_recon_report.tar.gz " .. results_dir)
    report_flow:run()

    print("🎉 ¡Flujo de reconocimiento finalizado! Reporte guardado en " .. target .. "_recon_report.tar.gz")
end

return { main = main }
