port module Main exposing (..)

import Browser
import FormatNumber as F
import FormatNumber.Locales as F
import Html exposing (Html)
import Html.Attributes as HA
import Html.Events as Event



-- PORTS


port setSettings : Settings -> Cmd msg


main : Program Settings Model Msg
main =
    Browser.element
        { init = init
        , update = update
        , subscriptions = \_ -> Sub.none
        , view = view
        }



-- MODEL


type alias Model =
    { isOpen : Bool
    , settings : Settings
    }


type alias Settings =
    { viscosity : Float
    , velocityDissipation : Float
    , diffusionIterations : Int
    , pressureIterations : Int
    }


init : Settings -> ( Model, Cmd Msg )
init initialSettings =
    let
        model =
            { isOpen = False
            , settings = initialSettings
            }
    in
    ( model, setSettings model.settings )



-- UPDATE


type Msg
    = Toggle
    | SaveSetting SettingMsg


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        Toggle ->
            ( { model | isOpen = not model.isOpen }, Cmd.none )

        SaveSetting settingToUpdate ->
            let
                newSettings =
                    updateSettings settingToUpdate model.settings
            in
            ( { model | settings = newSettings }
            , setSettings model.settings
            )


type SettingMsg
    = SetViscosity Float
    | SetVelocityDissipation Float
    | SetDiffusionIterations Int
    | SetPressureIterations Int


updateSettings : SettingMsg -> Settings -> Settings
updateSettings msg settings =
    case msg of
        SetViscosity newViscosity ->
            { settings | viscosity = newViscosity }

        SetVelocityDissipation newVelocityDissipation ->
            { settings | velocityDissipation = newVelocityDissipation }

        SetDiffusionIterations newDiffusionIterations ->
            { settings | diffusionIterations = newDiffusionIterations }

        SetPressureIterations newPressureIterations ->
            { settings | pressureIterations = newPressureIterations }



-- VIEW


view : Model -> Html Msg
view model =
    Html.div
        [ HA.id "controls" ]
        [ Html.div
            [ HA.class "controls"
            , HA.class <|
                if model.isOpen then
                    "visible"

                else
                    ""
            ]
            [ viewSettings model.settings ]
        , Html.footer []
            [ Html.ul [ HA.class "nav" ]
                [ Html.li []
                    [ Html.button
                        [ Event.onClick Toggle
                        , HA.class <|
                            if model.isOpen then
                                "active"

                            else
                                ""
                        ]
                        [ Html.text "Controls" ]
                    ]
                , Html.li []
                    [ Html.a
                        [ HA.href "https://github.com/sandydoo/" ]
                        [ Html.text "© 2021 Sander Melnikov" ]
                    ]
                , Html.li []
                    [ Html.a
                        [ HA.href "https://github.com/sandydoo/flux/blob/main/LICENSE" ]
                        [ Html.text "Licensed under MIT" ]
                    ]
                ]
            ]
        ]


viewSettings : Settings -> Html Msg
viewSettings settings =
    Html.ul
        [ HA.class "control-list" ]
        [ Html.div
            [ HA.class "col-span-2-md" ]
            [ Html.button [ Event.onClick Toggle, HA.class "text-secondary" ] [ Html.text "← Back" ]
            , Html.h2 [] [ Html.text "Fluid simulation" ]
            ]
        , Html.li [ HA.class "control" ]
            [ Html.label
                [ HA.for "viscosity" ]
                [ Html.h3
                    [ HA.class "control-title" ]
                    [ Html.text "Viscosity" ]
                , Html.p
                    [ HA.class "control-description" ]
                    [ Html.text
                        """
                            A viscous fluid resists any change to its velocity.
                            It spreads out and diffuses any force applied to it.
                            """
                    ]
                ]
            , Html.div [ HA.class "control-slider" ]
                [ Html.input
                    [ HA.id "viscosity"
                    , HA.type_ "range"
                    , HA.min "0.1"
                    , HA.max "4.0"
                    , HA.step "0.1"
                    , HA.value <| formatFloat settings.viscosity
                    , Event.onInput
                        (\value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetViscosity
                                |> SaveSetting
                        )
                    ]
                    []
                , Html.span
                    [ HA.class "control-value" ]
                    [ Html.text <| formatFloat settings.viscosity ]
                ]
            ]
        , Html.li [ HA.class "control" ]
            [ Html.label
                [ HA.for "velocity-dissipation" ]
                [ Html.h3
                    [ HA.class "control-title" ]
                    [ Html.text "Velocity dissipation" ]
                , Html.p
                    [ HA.class "control-description" ]
                    [ Html.text "Velocity should decrease, or dissipate, as it travels through a fluid." ]
                ]
            , Html.div [ HA.class "control-slider" ]
                [ Html.input
                    [ HA.id "velocity-dissipation"
                    , HA.type_ "range"
                    , HA.min "0.0"
                    , HA.max "2.0"
                    , HA.step "0.1"
                    , HA.value <| formatFloat settings.velocityDissipation
                    , Event.onInput
                        (\value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetVelocityDissipation
                                |> SaveSetting
                        )
                    ]
                    []
                , Html.span
                    [ HA.class "control-value" ]
                    [ Html.text <| formatFloat settings.velocityDissipation ]
                ]
            ]
        , Html.li [ HA.class "control" ]
            [ Html.label
                [ HA.for "diffusion-iterations" ]
                [ Html.h3
                    [ HA.class "control-title" ]
                    [ Html.text "Diffusion iterations" ]
                , Html.p
                    [ HA.class "control-description" ]
                    [ Html.text
                        """
                            Viscous fluids dissipate external forces and velocity through a process called “diffusion”.
                            Each iteration enchances this effect and the diffusion strength is controlled by the fluid’s viscosity.
                            """
                    ]
                ]
            , Html.div [ HA.class "control-slider" ]
                [ Html.input
                    [ HA.id "diffusion-iterations"
                    , HA.type_ "range"
                    , HA.min "0"
                    , HA.max "60"
                    , HA.step "1"
                    , HA.value <| String.fromInt settings.diffusionIterations
                    , Event.onInput
                        (\value ->
                            String.toInt value
                                |> Maybe.withDefault 0
                                |> SetDiffusionIterations
                                |> SaveSetting
                        )
                    ]
                    []
                , Html.span
                    [ HA.class "control-value" ]
                    [ Html.text <| String.fromInt settings.diffusionIterations ]
                ]
            ]
        , Html.li [ HA.class "control" ]
            [ Html.label
                [ HA.for "pressure-iterations" ]
                [ Html.h3
                    [ HA.class "control-title" ]
                    [ Html.text "Pressure iterations" ]
                , Html.p
                    [ HA.class "control-description" ]
                    [ Html.text
                        """
                            Applying a force to fluid creates pressure as the fluid pushes back.
                            Calculating pressure is expensive, but the fluid will look unrealistic with fewer than 20 iterations.
                            """
                    ]
                ]
            , Html.div [ HA.class "control-slider" ]
                [ Html.input
                    [ HA.id "pressure-iterations"
                    , HA.type_ "range"
                    , HA.min "0"
                    , HA.max "60"
                    , HA.step "1"
                    , HA.value <| String.fromInt settings.pressureIterations
                    , Event.onInput
                        (\value ->
                            String.toInt value
                                |> Maybe.withDefault 0
                                |> SetPressureIterations
                                |> SaveSetting
                        )
                    ]
                    []
                , Html.span
                    [ HA.class "control-value" ]
                    [ Html.text <| String.fromInt settings.pressureIterations ]
                ]
            ]
        ]


formatFloat : Float -> String
formatFloat value =
    F.format
        { decimals = F.Exact 1
        , system = F.Western
        , thousandSeparator = ","
        , decimalSeparator = "."
        , negativePrefix = "−"
        , negativeSuffix = ""
        , positivePrefix = ""
        , positiveSuffix = ""
        , zeroPrefix = ""
        , zeroSuffix = ""
        }
        value
