port module Main exposing (main)

import Browser
import Flux.Controls as Controls
import Flux.Settings as Settings
import Html exposing (Html)
import Html.Attributes as HA
import Json.Encode as Encode
import Set
import Ui.Panel as Panel
import Ui.Section as Section


port initFlux : Encode.Value -> Cmd msg


port setSettings : Encode.Value -> Cmd msg


main : Program () Model Msg
main =
    Browser.element
        { init = init
        , update = update
        , subscriptions = subscriptions
        , view = view
        }


type alias Model =
    { isOpen : Bool
    , settings : Settings.Settings
    }


type Msg
    = ToggleControls
    | SaveSetting Settings.SettingMsg


init : () -> ( Model, Cmd Msg )
init _ =
    let
        model =
            { isOpen = False
            , settings = Settings.default
            }
    in
    ( model
    , initFlux (Settings.encode model.settings)
    )


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        ToggleControls ->
            ( { model | isOpen = not model.isOpen }, Cmd.none )

        SaveSetting settingMsg ->
            let
                newSettings =
                    Settings.update settingMsg model.settings
            in
            ( { model | settings = newSettings }
            , setSettings (Settings.encode newSettings)
            )


subscriptions : Model -> Sub Msg
subscriptions model =
    Panel.subscriptions
        { isOpen = model.isOpen
        , onToggle = ToggleControls
        , openKeys = Set.fromList [ "c" ]
        , closeKeys = Set.fromList [ "c", "escape" ]
        }


view : Model -> Html Msg
view model =
    Controls.all model.settings
        |> List.map (Section.map SaveSetting)
        |> Panel.view
            { id = "controls-panel"
            , isOpen = model.isOpen
            , onToggle = ToggleControls
            , title = "Controls"
            , description = "Use this collection of knobs and dials to adjust the look and feel of the fluid simulation."
            , backLabel = "← Back"
            , triggerLabel = "🄲 Controls"
            , footerItems = footerItems
            }


footerItems : List (Html msg)
footerItems =
    [ Html.a
        [ HA.href "https://github.com/sandydoo/" ]
        [ Html.text "© 2022 Sander Melnikov" ]
    , Html.a
        [ HA.href "https://x.com/sandydoo/" ]
        [ Html.text "Follow me on X" ]
    , Html.a
        [ HA.href "https://sandydoo.gumroad.com/l/flux" ]
        [ Html.text "Buy this screensaver" ]
    ]
