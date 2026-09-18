use crate::gui::app::Message;

/// Genera las opciones del menú contextual para canciones y carpetas en la lista de reproducción.
pub fn get_item_context_menu_entries(
    linear_idx: usize,
) -> Vec<crate::gui::widgets::ContextMenuEntry<Message>> {
    use crate::gui::widgets::ContextMenuEntry;
    vec![
        ContextMenuEntry {
            label: "Reproducir".to_string(),
            icon: Some("play-circle-straight-outlined.svg".to_string()),
            action: Some(Message::PlaylistContextMenuPlay(linear_idx)),
        },
        ContextMenuEntry {
            label: "".to_string(),
            icon: None,
            action: None,
        },
        ContextMenuEntry {
            label: "Agregar archivo".to_string(),
            icon: Some("audio-file-outlined-straight.svg".to_string()),
            action: Some(Message::PlaylistAddFiles),
        },
        ContextMenuEntry {
            label: "Agregar carpeta".to_string(),
            icon: Some("folder-add-outlined.svg".to_string()),
            action: Some(Message::PlaylistAddFolder),
        },
        ContextMenuEntry {
            label: "".to_string(),
            icon: None,
            action: None,
        },
        ContextMenuEntry {
            label: "Información".to_string(),
            icon: Some("info-outlined-straight.svg".to_string()),
            action: Some(Message::NoOp),
        },
        ContextMenuEntry {
            label: "Ubicación del archivo".to_string(),
            icon: Some("folder-open-outlined.svg".to_string()),
            action: Some(Message::PlaylistShowFileLocation(linear_idx)),
        },
        ContextMenuEntry {
            label: "Mostrar en la biblioteca".to_string(),
            icon: Some("show-library-outlined-straight.svg".to_string()),
            action: Some(Message::PlaylistShowInLibrary(linear_idx)),
        },
        ContextMenuEntry {
            label: "".to_string(),
            icon: None,
            action: None,
        },
        ContextMenuEntry {
            label: "Enviar a otra lista".to_string(),
            icon: Some("send-straight.svg".to_string()),
            action: Some(Message::PlaylistRequestSubMenu(linear_idx)),
        },
        ContextMenuEntry {
            label: "Editor de Etiquetas".to_string(),
            icon: Some("edit-tag.svg".to_string()),
            action: Some(Message::NoOp),
        },
        ContextMenuEntry {
            label: "Convertidor de audio".to_string(),
            icon: Some("convert.svg".to_string()),
            action: Some(Message::NoOp),
        },
        ContextMenuEntry {
            label: "".to_string(),
            icon: None,
            action: None,
        },
        ContextMenuEntry {
            label: "Activar | Desactivar".to_string(),
            icon: Some("indicator-outlined.svg".to_string()),
            action: Some(Message::PlaylistToggleEnabled(linear_idx)),
        },
        ContextMenuEntry {
            label: "Plegar | Desplegar".to_string(),
            icon: Some("expand-collapse-straight.svg".to_string()),
            action: Some(Message::PlaylistToggleAllFolders),
        },
        ContextMenuEntry {
            label: "Eliminar de la lista".to_string(),
            icon: Some("delete-oulined.svg".to_string()),
            action: Some(Message::PlaylistDeleteSelection(linear_idx)),
        },
    ]
}
