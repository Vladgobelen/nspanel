-- nspanel: дефолтный конфиг.
-- Этот файл встроен в бинарник и копируется в ~/.config/nspanel/config.lua
-- при первом запуске (если пользовательского конфига ещё нет).
-- Правь ~/.config/nspanel/config.lua, а не этот файл.

return {
    panel = {
        height = 32,
        position = "top",  -- "top" | "bottom" | "left" | "right"
        show_borders = true,  -- рисовать 1px рамку вокруг виджетов (для отладки layout)
    },

    theme = {
        bg = 0xFF202020,
        fg = 0xFFFFFFFF,
        font = "/usr/share/fonts/corefonts/verdana.ttf",
        font_size = 14,
    },

    -- Пока пусто. Позже сюда будут попадать пути к Lua-модулям.
    modules = {},

    bindings = {
        right_click_panel = "menu",  -- ModuleId, который открывается по ПКМ на панель
    },

    -- Временно: пункты меню из конфига.
    -- Позже переедет в modules/menu.lua.
    menu_items = {
        {
            id = "settings",
            label = "Настройки",
            action = { type = "log", msg = "settings clicked" },
        },
        {
            id = "quit",
            label = "Выход",
            action = { type = "quit" },
        },
    },
}