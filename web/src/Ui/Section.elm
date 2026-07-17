module Ui.Section exposing
    ( Section
    , map
    , section
    , view
    )

import Html exposing (Html)
import Html.Attributes as HA
import Ui.Control as Control exposing (Control)


{-| A titled group of controls in a panel.

Sections render as a flat list so their controls remain direct children of the
panel's responsive grid.

-}
type Section msg
    = Section
        { id : String
        , title : String
        , controls : List (Control msg)
        }


section :
    { id : String
    , title : String
    , controls : List (Control msg)
    }
    -> Section msg
section =
    Section


map : (a -> b) -> Section a -> Section b
map toMessage (Section config) =
    Section
        { id = config.id
        , title = config.title
        , controls = List.map (Control.map toMessage) config.controls
        }


view : Section msg -> List (Html msg)
view (Section config) =
    Html.h2
        [ HA.id config.id
        , HA.class "col-span-2-md"
        ]
        [ Html.text config.title ]
        :: List.map Control.view config.controls
