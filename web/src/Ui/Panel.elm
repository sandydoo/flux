module Ui.Panel exposing
    ( Config
    , KeyboardConfig
    , subscriptions
    , view
    )

import Browser.Events as BrowserEvent
import Html exposing (Html)
import Html.Attributes as HA
import Html.Events as Event
import Json.Decode as Decode exposing (Decoder)
import Set exposing (Set)
import Ui.Section as Section exposing (Section)


type alias Config msg =
    { id : String
    , isOpen : Bool
    , onToggle : msg
    , title : String
    , description : String
    , backLabel : String
    , triggerLabel : String
    , footerItems : List (Html msg)
    }


type alias KeyboardConfig msg =
    { isOpen : Bool
    , onToggle : msg
    , openKeys : Set String
    , closeKeys : Set String
    }


view : Config msg -> List (Section msg) -> Html msg
view config sections =
    let
        titleId =
            config.id ++ "-title"

        descriptionId =
            config.id ++ "-description"
    in
    Html.div
        [ HA.class "flux-ui" ]
        [ Html.div
            [ HA.id config.id
            , HA.classList
                [ ( "control-panel", True )
                , ( "visible", config.isOpen )
                ]
            , HA.attribute "role" "dialog"
            , HA.attribute "aria-modal" "true"
            , HA.attribute "aria-labelledby" titleId
            , HA.attribute "aria-describedby" descriptionId
            , HA.tabindex -1
            , HA.hidden (not config.isOpen)
            ]
            [ Html.div
                [ HA.class "control-container" ]
                [ Html.div
                    [ HA.class "control-list" ]
                    (viewHeader config titleId descriptionId
                        :: List.concatMap Section.view sections
                    )
                ]
            ]
        , viewFooter config
        ]


subscriptions : KeyboardConfig msg -> Sub msg
subscriptions config =
    BrowserEvent.onKeyDown
        (Decode.field "key" Decode.string
            |> Decode.andThen (toggleOnKey config)
        )


viewHeader : Config msg -> String -> String -> Html msg
viewHeader config titleId descriptionId =
    Html.div
        [ HA.class "col-span-2-md" ]
        [ Html.button
            [ Event.onClick config.onToggle
            , HA.type_ "button"
            , HA.class "text-secondary"
            ]
            [ Html.text config.backLabel ]
        , Html.h2
            [ HA.id titleId ]
            [ Html.text config.title ]
        , Html.p
            [ HA.id descriptionId
            , HA.class "control-description"
            ]
            [ Html.text config.description ]
        ]


viewFooter : Config msg -> Html msg
viewFooter config =
    Html.footer []
        [ Html.ul
            [ HA.class "nav" ]
            (Html.li []
                [ Html.button
                    [ Event.onClick config.onToggle
                    , HA.type_ "button"
                    , HA.classList
                        [ ( "active", config.isOpen )
                        , ( "whitespace-nowrap", True )
                        ]
                    ]
                    [ Html.text config.triggerLabel ]
                ]
                :: List.map (\item -> Html.li [] [ item ]) config.footerItems
            )
        ]


toggleOnKey : KeyboardConfig msg -> String -> Decoder msg
toggleOnKey config key =
    let
        activeKeys =
            if config.isOpen then
                config.closeKeys

            else
                config.openKeys
    in
    if Set.member (String.toLower key) activeKeys then
        Decode.succeed config.onToggle

    else
        Decode.fail ""
