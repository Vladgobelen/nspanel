#!/bin/bash
# find_icons.sh — находит окна, их .desktop-файлы и пути к иконкам.
# Использование: ./find_icons.sh

# 1. Получаем список окон.
# Сначала пробуем через nsbridge (если он запущен).
SOCKET="/run/user/$(id -u)/nsbridge.sock"
if [ -S "$SOCKET" ]; then
    # Отправляем запрос {"type":"request"} в формате фрейма (4 байта длины + JSON)
    REQ='{"type":"request"}'
    LEN=$(printf '%08x' ${#REQ})
    # Используем socat или nc, но лучше чистый bash через /dev/tcp (для unix-сокетов нужен socat)
    if command -v socat >/dev/null; then
        RESPONSE=$(echo -n "$(printf "\\x${LEN:0:2}\\x${LEN:2:2}\\x${LEN:4:2}\\x${LEN:6:2}")${REQ}" | socat - UNIX-CONNECT:"$SOCKET" | tail -c +5)
    else
        echo "socat не найден, откатываемся на xprop..."
        RESPONSE=""
    fi
else
    RESPONSE=""
fi

# Если nsbridge не ответил, используем xprop (менее надежно, но работает без моста).
if [ -z "$RESPONSE" ]; then
    echo "Использую xprop для поиска окон..."
    WINDOWS=$(xprop -root _NET_CLIENT_LIST | grep -o '0x[0-9a-f]*')
    for win in $WINDOWS; do
        CLASS=$(xprop -id "$win" WM_CLASS 2>/dev/null | awk -F'"' '{print $4}')
        if [ -n "$CLASS" ]; then
            EXE=$(readlink /proc/$(xprop -id "$win" _NET_WM_PID 2>/dev/null | awk '{print $3}')/exe 2>/dev/null)
            echo "WINDOW: $win CLASS: $CLASS EXE: ${EXE:-unknown}"
            RESPONSE="$RESPONSE\nWINDOW: $win CLASS: $CLASS EXE: ${EXE:-unknown}"
        fi
    done
else
    # Парсим JSON от nsbridge.
    echo "Использую nsbridge..."
    # Простой парсинг без jq (jq может не быть).
    # Ищем группы и их окна.
    echo "$RESPONSE" | grep -o '"key":"[^"]*"\|"class":"[^"]*"\|"instance":"[^"]*"\|"exe_path":"[^"]*"' | paste - - - -
fi

echo -e "\n=== Поиск иконок для найденных классов ===\n"

# 2. Ищем иконки для каждого класса.
# Проходим по всем .desktop файлам (системным и пользовательским).
for desktop in /usr/share/applications/*.desktop ~/.local/share/applications/*.desktop; do
    [ -f "$desktop" ] || continue
    
    # Извлекаем имя иконки и exec.
    ICON_NAME=$(grep -m1 "^Icon=" "$desktop" | cut -d'=' -f2-)
    EXEC_CMD=$(grep -m1 "^Exec=" "$desktop" | cut -d'=' -f2- | awk '{print $1}')
    
    if [ -z "$ICON_NAME" ]; then
        continue
    fi

    # Ищем PNG или SVG по стандартным путям.
    # Сначала проверяем, не абсолютный ли путь.
    if [[ "$ICON_NAME" == /* ]] && [ -f "$ICON_NAME" ]; then
        echo "FOUND: $ICON_NAME (from $desktop)"
        continue
    fi

    # Ищем в hicolor (основной стандарт).
    FOUND=""
    for size in 48x48 32x32 64x64 128x128 256x256; do
        for ext in png svg; do
            # Проверяем системные пути.
            for base in /usr/share/icons/hicolor /usr/share/icons/Adwaita /usr/share/pixmaps ~/.local/share/icons; do
                f="$base/$size/apps/$ICON_NAME.$ext"
                if [ -f "$f" ]; then
                    FOUND="$f"
                    break 2
                fi
                f="$base/$ICON_NAME.$ext"
                if [ -f "$f" ]; then
                    FOUND="$f"
                    break 2
                fi
            done
        done
    done

    # Если не нашли по размеру, ищем в scalable (SVG).
    if [ -z "$FOUND" ]; then
        for base in /usr/share/icons/hicolor /usr/share/icons/Adwaita ~/.local/share/icons; do
            f="$base/scalable/apps/$ICON_NAME.svg"
            if [ -f "$f" ]; then
                FOUND="$f"
                break
            fi
        done
    fi

    if [ -n "$FOUND" ]; then
        echo "DESKTOP: $(basename "$desktop")"
        echo "  Icon: $ICON_NAME"
        echo "  Path: $FOUND"
        echo "  Exec: $EXEC_CMD"
        echo ""
    fi
done