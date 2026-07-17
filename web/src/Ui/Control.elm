module Ui.Control exposing
    ( ButtonOption
    , Control
    , GalleryItem
    , SelectOption
    , SliderConfig
    , buttonGroup
    , custom
    , gallery
    , group
    , map
    , select
    , slider
    , view
    )

import Html exposing (Html)
import Html.Attributes as HA
import Html.Events as Event


{-| A rendered control whose message type can be adapted with `map`.

The constructor is intentionally opaque so every standard control keeps the
same markup and class contract.

-}
type Control msg
    = Control (Html msg)


type alias SliderConfig value msg =
    { id : String
    , title : String
    , description : String
    , min : value
    , max : value
    , step : value
    , value : value
    , toString : value -> String
    , onInput : String -> msg
    }


type alias SelectOption =
    { value : String
    , label : String
    , disabled : Bool
    }


type alias ButtonOption msg =
    { id : String
    , label : String
    , active : Bool
    , onClick : msg
    }


type alias GalleryItem msg =
    { id : String
    , label : String
    , imageSrc : String
    , imageAlt : String
    , active : Bool
    , onClick : msg
    }


slider : SliderConfig value msg -> Control msg
slider config =
    Control <|
        Html.div [ HA.class "control" ]
            [ Html.label
                [ HA.for config.id ]
                [ controlTitle config.title
                , controlDescription config.description
                , Html.div [ HA.class "control-slider" ]
                    [ Html.input
                        [ HA.id config.id
                        , HA.type_ "range"
                        , HA.min (config.toString config.min)
                        , HA.max (config.toString config.max)
                        , HA.step (config.toString config.step)
                        , HA.value (config.toString config.value)
                        , Event.onInput config.onInput
                        ]
                        []
                    , Html.span
                        [ HA.class "control-value" ]
                        [ Html.text (config.toString config.value) ]
                    ]
                ]
            ]


select :
    { id : String
    , title : String
    , description : String
    , value : String
    , options : List SelectOption
    , onInput : String -> msg
    }
    -> Control msg
select config =
    Control <|
        Html.div [ HA.class "control" ]
            [ Html.label
                [ HA.for config.id ]
                [ controlTitle config.title
                , controlDescription config.description
                , Html.div [ HA.class "control-select" ]
                    [ Html.select
                        [ HA.id config.id
                        , HA.value config.value
                        , Event.onInput config.onInput
                        ]
                        (List.map viewSelectOption config.options)
                    ]
                ]
            ]


buttonGroup :
    { id : String
    , label : String
    , options : List (ButtonOption msg)
    }
    -> Control msg
buttonGroup config =
    Control <|
        Html.div
            [ HA.id config.id
            , HA.class "button-group col-span-2-md"
            , HA.attribute "role" "group"
            , HA.attribute "aria-label" config.label
            ]
            (List.map viewButtonOption config.options)


gallery :
    { id : String
    , label : String
    , items : List (GalleryItem msg)
    }
    -> Control msg
gallery config =
    Control <|
        Html.div
            [ HA.id config.id
            , HA.class "col-span-2-md"
            , HA.attribute "role" "group"
            , HA.attribute "aria-label" config.label
            , HA.style "overflow-x" "scroll"
            ]
            [ Html.div
                [ HA.style "white-space" "nowrap" ]
                (List.map viewGalleryItem config.items)
            ]


group :
    { id : String
    , title : String
    , controls : List (Control msg)
    }
    -> Control msg
group config =
    Control <|
        Html.div
            [ HA.id config.id
            , HA.class "control-list-single"
            ]
            (Html.div [] [ Html.h4 [] [ Html.text config.title ] ]
                :: List.map view config.controls
            )


custom : Html msg -> Control msg
custom =
    Control


map : (a -> b) -> Control a -> Control b
map toMessage (Control html) =
    Control (Html.map toMessage html)


view : Control msg -> Html msg
view (Control html) =
    html


controlTitle : String -> Html msg
controlTitle title =
    Html.h3
        [ HA.class "control-title" ]
        [ Html.text title ]


controlDescription : String -> Html msg
controlDescription description =
    Html.p
        [ HA.class "control-description" ]
        [ Html.text description ]


viewSelectOption : SelectOption -> Html msg
viewSelectOption option =
    Html.option
        [ HA.value option.value
        , HA.disabled option.disabled
        ]
        [ Html.text option.label ]


viewButtonOption : ButtonOption msg -> Html msg
viewButtonOption option =
    Html.button
        [ HA.id option.id
        , HA.type_ "button"
        , HA.attribute "aria-pressed" (boolToString option.active)
        , HA.classList
            [ ( "button", True )
            , ( "active", option.active )
            ]
        , Event.onClick option.onClick
        ]
        [ Html.text option.label ]


viewGalleryItem : GalleryItem msg -> Html msg
viewGalleryItem item =
    Html.button
        [ HA.id item.id
        , HA.type_ "button"
        , HA.class "gallery-item"
        , HA.attribute "aria-pressed" (boolToString item.active)
        , Event.onClick item.onClick
        ]
        [ Html.img
            [ HA.src item.imageSrc
            , HA.alt item.imageAlt
            , HA.attribute "loading" "lazy"
            , HA.attribute "decoding" "async"
            , HA.classList
                [ ( "gallery-icon", True )
                , ( "active", item.active )
                ]
            , HA.style "object-fit" "cover"
            ]
            []
        , Html.span
            [ HA.style "text-align" "center"
            , HA.style "width" "100%"
            , HA.style "margin-top" "8px"
            ]
            [ Html.text item.label ]
        ]


boolToString : Bool -> String
boolToString value =
    if value then
        "true"

    else
        "false"
