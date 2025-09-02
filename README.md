# Hackflow

Motor de orquestación de flujos de trabajo escrito en **Rust** con scripts de
**Lua**. Permite definir tareas que se ejecutan de forma secuencial o en
paralelo utilizando un grafo acíclico dirigido.

## Características

- Sistema de tareas extensible con soporte para dependencias.
- Paso de datos entre tareas: la salida de una tarea puede modificar el
  comportamiento de la siguiente. Por ejemplo, `RunCommandTask` ahora añade
  argumentos basados en resultados de tareas previas como `DnsLookupTask`.

## Uso

```bash
$ hackflow --help
```

### Inicializar un nuevo entorno

```bash
$ hackflow init
```

Esto crea una carpeta `flows` y un archivo `config.toml` con opciones
configurables.

### Ejecutar un flujo

```bash
$ hackflow run ruta/al_flujo.lua arg1 arg2
```

Los argumentos posteriores al archivo se pasan al flujo de trabajo.
