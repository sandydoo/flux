port module Main exposing (..)

import Browser
import Browser.Events as BrowserEvent
import FormatNumber as F
import FormatNumber.Locales as F
import Html exposing (Html)
import Html.Attributes as HA
import Html.Events as Event
import Json.Decode as Decode
import Json.Decode.Pipeline as Decode
import Json.Encode as Encode



-- PORTS


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



-- MODEL


type alias Model =
    { isOpen : Bool
    , settings : Settings
    }


type alias Settings =
    { viscosity : Float
    , velocityDissipation : Float
    , startingPressure : Float
    , fluidSize : Int
    , fluidSimulationFrameRate : Int
    , diffusionIterations : Int
    , pressureIterations : Int
    , colorScheme : ColorScheme
    , lineLength : Float
    , lineWidth : Float
    , lineBeginOffset : Float
    , lineFadeOutLength : Float
    , springStiffness : Float
    , springVariance : Float
    , springMass : Float
    , springDamping : Float
    , springRestLength : Float
    , maxLineVelocity : Float
    , advectionDirection : AdvectionDirection
    , adjustAdvection : Float
    , gridSpacing : Int
    , viewScale : Float
    , noiseChannel1 : Noise
    , noiseChannel2 : Noise
    }


type AdvectionDirection
    = Forward
    | Reverse


type ColorScheme
    = Plasma
    | Peacock
    | Poolside
    | Pollen


type alias Noise =
    { scale : Float
    , multiplier : Float
    , offset1 : Float
    , offset2 : Float
    , offsetIncrement : Float
    , delay : Float
    , blendDuration : Float
    , blendThreshold : Float
    , blendMethod : BlendMethod
    }


type BlendMethod
    = Curl
    | Wiggle


defaultSettings : Settings
defaultSettings =
    { viscosity = 1.5
    , velocityDissipation = 0.0
    , startingPressure = 0.0
    , fluidSize = 128
    , fluidSimulationFrameRate = 30
    , colorScheme = Peacock
    , diffusionIterations = 30
    , pressureIterations = 60
    , lineLength = 220.0
    , lineWidth = 6.0
    , lineBeginOffset = 0.5
    , lineFadeOutLength = 0.005
    , springVariance = 0.00
    , springStiffness = 0.4
    , springMass = 2.8
    , springDamping = 20.0
    , springRestLength = 0.0
    , maxLineVelocity = 1.0
    , advectionDirection = Forward
    , viewScale = 1.2
    , adjustAdvection = 60.0
    , gridSpacing = 20
    , noiseChannel1 =
        { scale = 0.9
        , multiplier = 0.1
        , offset1 = 2.0
        , offset2 = 10.0
        , offsetIncrement = 0.05
        , delay = 3.0
        , blendDuration = 3.0
        , blendThreshold = 0.3
        , blendMethod = Curl
        }
    , noiseChannel2 =
        { scale = 25.0
        , multiplier = 0.1
        , offset1 = 3.0
        , offset2 = 2.0
        , offsetIncrement = 0.02
        , delay = 0.2
        , blendDuration = 1.0
        , blendThreshold = 0.0
        , blendMethod = Curl
        }
    }


init : () -> ( Model, Cmd Msg )
init _ =
    let
        model =
            { isOpen = False
            , settings = defaultSettings
            }
    in
    ( model
    , initFlux (encodeSettings model.settings)
    )



-- UPDATE


type Msg
    = ToggleControls
    | SaveSetting SettingMsg


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        ToggleControls ->
            ( { model | isOpen = not model.isOpen }, Cmd.none )

        SaveSetting settingToUpdate ->
            let
                newSettings =
                    updateSettings settingToUpdate model.settings
            in
            ( { model | settings = newSettings }
            , setSettings (encodeSettings newSettings)
            )


type SettingMsg
    = SetViscosity Float
    | SetVelocityDissipation Float
    | SetStartingPressure Float
    | SetDiffusionIterations Int
    | SetPressureIterations Int
    | SetColorScheme ColorScheme
    | SetLineLength Float
    | SetLineWidth Float
    | SetLineBeginOffset Float
    | SetLineFadeOutLength Float
    | SetSpringStiffness Float
    | SetSpringVariance Float
    | SetSpringMass Float
    | SetSpringDamping Float
    | SetSpringRestLength Float
    | SetAdvectionDirection AdvectionDirection
    | SetAdjustAdvection Float
    | SetNoiseChannel1 NoiseMsg
    | SetNoiseChannel2 NoiseMsg


type NoiseMsg
    = SetNoiseScale Float
    | SetNoiseMultiplier Float
    | SetNoiseOffset1 Float
    | SetNoiseOffset2 Float
    | SetNoiseOffsetIncrement Float
    | SetNoiseDelay Float
    | SetNoiseBlendDuration Float
    | SetNoiseBlendThreshold Float


updateSettings : SettingMsg -> Settings -> Settings
updateSettings msg settings =
    case msg of
        SetViscosity newViscosity ->
            { settings | viscosity = newViscosity }

        SetVelocityDissipation newVelocityDissipation ->
            { settings | velocityDissipation = newVelocityDissipation }

        SetStartingPressure newPressure ->
            { settings | startingPressure = newPressure }

        SetDiffusionIterations newDiffusionIterations ->
            { settings | diffusionIterations = newDiffusionIterations }

        SetPressureIterations newPressureIterations ->
            { settings | pressureIterations = newPressureIterations }

        SetColorScheme newColorScheme ->
            { settings | colorScheme = newColorScheme }

        SetLineLength newLineLength ->
            { settings | lineLength = newLineLength }

        SetLineWidth newLineWidth ->
            { settings | lineWidth = newLineWidth }

        SetLineBeginOffset newLineBeginOffset ->
            { settings | lineBeginOffset = newLineBeginOffset }

        SetLineFadeOutLength newLineFadeOutLength ->
            { settings | lineFadeOutLength = newLineFadeOutLength / settings.lineLength }

        SetSpringStiffness newSpringStiffness ->
            { settings | springStiffness = newSpringStiffness }

        SetSpringVariance newSpringVariance ->
            { settings | springVariance = newSpringVariance }

        SetSpringMass newSpringMass ->
            { settings | springMass = newSpringMass }

        SetSpringDamping newSpringDamping ->
            { settings | springDamping = newSpringDamping }

        SetSpringRestLength newSpringRestLength ->
            { settings | springRestLength = newSpringRestLength }

        SetAdvectionDirection newDirection ->
            { settings | advectionDirection = newDirection }

        SetAdjustAdvection newAdjustAdvection ->
            { settings | adjustAdvection = newAdjustAdvection }

        SetNoiseChannel1 noiseMsg ->
            { settings | noiseChannel1 = updateNoise noiseMsg settings.noiseChannel1 }

        SetNoiseChannel2 noiseMsg ->
            { settings | noiseChannel2 = updateNoise noiseMsg settings.noiseChannel2 }


updateNoise : NoiseMsg -> Noise -> Noise
updateNoise msg noise =
    case msg of
        SetNoiseScale newScale ->
            { noise | scale = newScale }

        SetNoiseMultiplier newMultiplier ->
            { noise | multiplier = newMultiplier }

        SetNoiseOffset1 newOffset ->
            { noise | offset1 = newOffset }

        SetNoiseOffset2 newOffset ->
            { noise | offset2 = newOffset }

        SetNoiseOffsetIncrement newOffsetIncrement ->
            { noise | offsetIncrement = newOffsetIncrement }

        SetNoiseDelay newDelay ->
            { noise | delay = newDelay }

        SetNoiseBlendDuration newBlendDuration ->
            { noise | blendDuration = newBlendDuration }

        SetNoiseBlendThreshold newBlendThreshold ->
            { noise | blendThreshold = newBlendThreshold }



-- SUBSCRIPTIONS


subscriptions : Model -> Sub Msg
subscriptions { isOpen } =
    if isOpen then
        Sub.batch
            [ BrowserEvent.onKeyDown (decodeKeyCode "Escape" ToggleControls)
            , BrowserEvent.onKeyDown (decodeKeyCode "KeyC" ToggleControls)
            ]

    else
        BrowserEvent.onKeyDown (decodeKeyCode "KeyC" ToggleControls)



-- VIEW


type alias Control value =
    { title : String
    , description : String
    , input : Input value
    }


type Input number
    = Slider
        { min : number
        , max : number
        , step : number
        , value : number
        , onInput : String -> Msg
        , toString : number -> String
        }


view : Model -> Html Msg
view model =
    let
        classNameWhen className condition =
            if condition then
                className

            else
                ""
    in
    Html.div []
        [ Html.div
            [ HA.class "control-panel"
            , HA.class (classNameWhen "visible" model.isOpen)
            , HA.attribute "role" "dialog"
            , HA.attribute "aria-modal" "true"
            , HA.attribute "aria-labelledby" "control-title"
            , HA.attribute "aria-describedby" "control-description"
            , HA.tabindex -1
            , HA.hidden (not model.isOpen)
            ]
            [ Html.div
                [ HA.class "control-container" ]
                [ viewSettings model.settings ]
            ]
        , Html.footer []
            [ Html.ul [ HA.class "nav" ]
                [ Html.li []
                    [ Html.button
                        [ Event.onClick ToggleControls
                        , HA.type_ "button"
                        , HA.class (classNameWhen "active" model.isOpen)
                        , HA.class "whitespace-nowrap"
                        ]
                        [ Html.text "🄲 Controls" ]
                    ]
                , Html.li []
                    [ Html.a
                        [ HA.href "https://github.com/sandydoo/" ]
                        [ Html.text "© 2021 Sander Melnikov" ]
                    ]
                , Html.li []
                    [ Html.a
                        [ HA.href "https://twitter.com/sandy_doo/" ]
                        [ Html.text "Follow me on Twitter" ]
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
            [ Html.button
                [ Event.onClick ToggleControls
                , HA.type_ "button"
                , HA.class "text-secondary"
                ]
                [ Html.text "← Back" ]
            , Html.h2 [ HA.id "control-title" ] [ Html.text "Controls" ]
            , Html.p
                [ HA.class "control-description" ]
                [ Html.text
                    """
                    Use this collection of knobs and dials to adjust the look and feel of the fluid simulation.
                    """
                ]
            ]
        , Html.h2 [ HA.class "col-span-2-md" ] [ Html.text "Colors" ]
        , viewButtonGroup (SetColorScheme >> SaveSetting)
            settings.colorScheme
            [ ( "Plasma", Plasma )
            , ( "Peacock", Peacock )
            , ( "Poolside", Poolside )
            , ( "Pollen", Pollen )
            ]
        , Html.div
            [ HA.class "col-span-2-md" ]
            [ Html.h2 [] [ Html.text "Advection" ]
            , Html.p
                [ HA.class "control-description" ]
                [ Html.text
                    """
                    Advection is the transport of some substance by motion of a fluid, and that substance is the field of lines.
                    In “forward” mode, the lines point in the direction of fluid movement and tend to curl outwards. And in “reverse”, the lines create whirlpools as they spiral inwards.
                    """
                ]
            ]
        , viewButtonGroup (SetAdvectionDirection >> SaveSetting)
            settings.advectionDirection
            [ ( "Forward", Forward )
            , ( "Reverse", Reverse )
            ]
        , Html.h2
            [ HA.class "col-span-2-md" ]
            [ Html.text "Look" ]
        , viewControl <|
            Control
                "Line length"
                """
                The maximum length of a line.
                """
                (Slider
                    { min = 1.0
                    , max = 500.0
                    , step = 1.0
                    , value = settings.lineLength
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetLineLength
                                |> SaveSetting
                    , toString = formatFloat 0
                    }
                )
        , viewControl <|
            Control
                "Line width"
                """
                The maximum width of a line.
                """
                (Slider
                    { min = 1.0
                    , max = 20.0
                    , step = 0.1
                    , value = settings.lineWidth
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetLineWidth
                                |> SaveSetting
                    , toString = formatFloat 1
                    }
                )
        , viewControl <|
            Control
                "Line fade offset"
                """
                The point along a line when it begins to fade out.
                """
                (Slider
                    { min = 0.0
                    , max = 1.0
                    , step = 0.01
                    , value = settings.lineBeginOffset
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetLineBeginOffset
                                |> SaveSetting
                    , toString = formatFloat 2
                    }
                )
        , viewControl <|
            let
                toAbsoluteLength : Float -> Float
                toAbsoluteLength offset =
                    settings.lineLength * offset
            in
            Control
                "Fog level"
                """
                Adjust the height of the fog which consumes shorter lines.
                """
                (Slider
                    { min = 0.0
                    , max = toAbsoluteLength 0.5
                    , step = 0.1
                    , value = toAbsoluteLength settings.lineFadeOutLength
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetLineFadeOutLength
                                |> SaveSetting
                    , toString = formatFloat 1
                    }
                )
        , viewControl <|
            Control
                "Stiffness"
                """
                The stiffness of the line determines the amount of force needed to extend it.
                """
                (Slider
                    { min = 0.0
                    , max = 1.0
                    , step = 0.01
                    , value = settings.springStiffness
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetSpringStiffness
                                |> SaveSetting
                    , toString = formatFloat 2
                    }
                )
        , viewControl <|
            Control
                "Mass"
                """
                Adjust the weight of each line. More mass means more momentum and more sluggish movement.
                """
                (Slider
                    { min = 1.0
                    , max = 20.0
                    , step = 0.1
                    , value = settings.springMass
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetSpringMass
                                |> SaveSetting
                    , toString = formatFloat 1
                    }
                )
        , viewControl <|
            Control
                "Damping"
                """
                Dampen line oscillations.
                """
                (Slider
                    { min = 0.0
                    , max = 20.0
                    , step = 0.1
                    , value = settings.springDamping
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetSpringDamping
                                |> SaveSetting
                    , toString = formatFloat 1
                    }
                )
        , viewControl <|
            Control
                "Variance"
                """
                Give each line a slightly different amount of mass.
                """
                (Slider
                    { min = 0.0
                    , max = 1.0
                    , step = 0.01
                    , value = settings.springVariance
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetSpringVariance
                                |> SaveSetting
                    , toString = formatFloat 2
                    }
                )
        , viewControl <|
            Control
                "Resting length"
                """
                The length of a line at rest, when no forces are applied to it.
                """
                (Slider
                    { min = 0.0
                    , max = 1.0
                    , step = 0.01
                    , value = settings.springRestLength
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetSpringRestLength
                                |> SaveSetting
                    , toString = formatFloat 2
                    }
                )
        , viewControl <|
            Control
                "Advection speed"
                """
                Adjust how quickly the lines respond to changes in the fluid.
                """
                (Slider
                    { min = 0.1
                    , max = 50.0
                    , step = 0.1
                    , value = settings.adjustAdvection
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetAdjustAdvection
                                |> SaveSetting
                    , toString = formatFloat 1
                    }
                )
        , Html.h2 [ HA.class "col-span-2-md" ] [ Html.text "Fluid simulation" ]
        , viewControl <|
            Control
                "Viscosity"
                """
                A viscous fluid resists any change to its velocity.
                It spreads out and diffuses any force applied to it.
                """
                (Slider
                    { min = 0.1
                    , max = 4.0
                    , step = 0.1
                    , value = settings.viscosity
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetViscosity
                                |> SaveSetting
                    , toString = formatFloat 1
                    }
                )
        , viewControl <|
            Control
                "Velocity dissipation"
                """
                Velocity should decrease, or dissipate, as it travels through a fluid.
                """
                (Slider
                    { min = 0.0
                    , max = 2.0
                    , step = 0.1
                    , value = settings.velocityDissipation
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetVelocityDissipation
                                |> SaveSetting
                    , toString = formatFloat 1
                    }
                )
        , viewControl <|
            Control
                "Starting pressure"
                """
                The amount of fluid pressure we assume before actually calculating pressure.
                """
                (Slider
                    { min = 0.0
                    , max = 1.0
                    , step = 0.1
                    , value = settings.startingPressure
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetStartingPressure
                                |> SaveSetting
                    , toString = formatFloat 1
                    }
                )
        , viewControl <|
            Control
                "Diffusion iterations"
                """
                Viscous fluids dissipate velocity through a process called “diffusion”.
                Each iteration enchances this effect and the diffusion strength is controlled by the fluid’s viscosity.
                """
                (Slider
                    { min = 0
                    , max = 30
                    , step = 1
                    , value = settings.diffusionIterations
                    , onInput =
                        \value ->
                            String.toInt value
                                |> Maybe.withDefault 0
                                |> SetDiffusionIterations
                                |> SaveSetting
                    , toString = String.fromInt
                    }
                )
        , viewControl <|
            Control
                "Pressure iterations"
                """
                Applying a force to fluid creates pressure as the fluid pushes back.
                Calculating pressure is expensive, but the fluid will look unrealistic with fewer than 20 iterations.
                """
                (Slider
                    { min = 0
                    , max = 60
                    , step = 1
                    , value = settings.pressureIterations
                    , onInput =
                        \value ->
                            String.toInt value
                                |> Maybe.withDefault 0
                                |> SetPressureIterations
                                |> SaveSetting
                    , toString = String.fromInt
                    }
                )
        , Html.h2
            [ HA.class "col-span-2-md" ]
            [ Html.text "Noise" ]
        , viewNoiseChannel "Channel 1" SetNoiseChannel1 settings.noiseChannel1
        , viewNoiseChannel "Channel 2" SetNoiseChannel2 settings.noiseChannel2
        ]


viewButtonGroup : (value -> msg) -> value -> List ( String, value ) -> Html msg
viewButtonGroup onClick active options =
    let
        isActive : value -> String
        isActive value =
            if value == active then
                "active"

            else
                ""
    in
    Html.div [ HA.class "button-group col-span-2-md" ] <|
        List.map
            (\( name, value ) ->
                Html.button
                    [ HA.type_ "button"
                    , HA.class "button"
                    , HA.class (isActive value)
                    , Event.onClick (onClick value)
                    ]
                    [ Html.text name ]
            )
            options


viewNoiseChannel title setNoiseChannel noiseChannel =
    Html.div
        [ HA.class "control-list-single" ]
        [ Html.div []
            [ Html.h4 [] [ Html.text title ]
            , Html.p [ HA.class "control-description" ] [ Html.text "Simplex noise" ]
            ]
        , viewControl <|
            Control
                "Scale"
                ""
                (Slider
                    { min = 0.1
                    , max = 30.0
                    , step = 0.1
                    , value = noiseChannel.scale
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetNoiseScale
                                |> setNoiseChannel
                                |> SaveSetting
                    , toString = formatFloat 1
                    }
                )
        , viewControl <|
            Control
                "Strength"
                ""
                (Slider
                    { min = 0.0
                    , max = 1.0
                    , step = 0.01
                    , value = noiseChannel.multiplier
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetNoiseMultiplier
                                |> setNoiseChannel
                                |> SaveSetting
                    , toString = formatFloat 2
                    }
                )
        , viewControl <|
            Control
                "Offset 1"
                ""
                (Slider
                    { min = 0.0
                    , max = 100.0
                    , step = 1.0
                    , value = noiseChannel.offset1
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetNoiseOffset1
                                |> setNoiseChannel
                                |> SaveSetting
                    , toString = formatFloat 1
                    }
                )
        , viewControl <|
            Control
                "Offset 2"
                ""
                (Slider
                    { min = 0.0
                    , max = 100.0
                    , step = 1.0
                    , value = noiseChannel.offset2
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetNoiseOffset2
                                |> setNoiseChannel
                                |> SaveSetting
                    , toString = formatFloat 1
                    }
                )
        , viewControl <|
            Control
                "Offset increment"
                ""
                (Slider
                    { min = 0.0
                    , max = 1.0
                    , step = 0.01
                    , value = noiseChannel.offsetIncrement
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetNoiseOffsetIncrement
                                |> setNoiseChannel
                                |> SaveSetting
                    , toString = formatFloat 2
                    }
                )
        , viewControl <|
            Control
                "Delay"
                ""
                (Slider
                    { min = 0.0
                    , max = 10.0
                    , step = 0.1
                    , value = noiseChannel.delay
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 1.0
                                |> SetNoiseDelay
                                |> setNoiseChannel
                                |> SaveSetting
                    , toString = formatFloat 1
                    }
                )
        , viewControl <|
            Control
                "Blend duration"
                ""
                (Slider
                    { min = 0.1
                    , max = 10.0
                    , step = 0.1
                    , value = noiseChannel.blendDuration
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetNoiseBlendDuration
                                |> setNoiseChannel
                                |> SaveSetting
                    , toString = formatFloat 1
                    }
                )
        , viewControl <|
            Control
                "Blend threshold"
                ""
                (Slider
                    { min = 0.0
                    , max = 0.6
                    , step = 0.01
                    , value = noiseChannel.blendThreshold
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetNoiseBlendThreshold
                                |> setNoiseChannel
                                |> SaveSetting
                    , toString = formatFloat 2
                    }
                )
        ]


viewControl : Control number -> Html Msg
viewControl { title, description, input } =
    let
        id =
            toKebabcase title
    in
    Html.li [ HA.class "control" ]
        [ Html.label
            [ HA.for id ]
            [ Html.h3
                [ HA.class "control-title" ]
                [ Html.text title ]
            , Html.p
                [ HA.class "control-description" ]
                [ Html.text description ]
            , Html.div [ HA.class "control-slider" ] <|
                case input of
                    Slider slider ->
                        [ Html.input
                            [ HA.id id
                            , HA.type_ "range"
                            , HA.min <| slider.toString slider.min
                            , HA.max <| slider.toString slider.max
                            , HA.step <| slider.toString slider.step
                            , HA.value <| slider.toString slider.value
                            , Event.onInput slider.onInput
                            ]
                            []
                        , Html.span
                            [ HA.class "control-value" ]
                            [ Html.text <| slider.toString slider.value ]
                        ]
            ]
        ]


formatFloat : Int -> Float -> String
formatFloat decimals value =
    F.format
        { decimals = F.Exact decimals
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


toKebabcase : String -> String
toKebabcase =
    let
        -- This only converts titles separated by spaces
        kebabify char =
            if char == ' ' then
                '-'

            else
                Char.toLower char
    in
    String.map kebabify



-- JSON


decodeKeyCode : String -> msg -> Decode.Decoder msg
decodeKeyCode key msg =
    Decode.field "code" Decode.string
        |> Decode.andThen
            (\string ->
                if string == key then
                    Decode.succeed msg

                else
                    Decode.fail ""
            )


encodeSettings : Settings -> Encode.Value
encodeSettings settings =
    Encode.object
        [ ( "viscosity", Encode.float settings.viscosity )
        , ( "velocityDissipation", Encode.float settings.velocityDissipation )
        , ( "startingPressure", Encode.float settings.startingPressure )
        , ( "fluidSize", Encode.int settings.fluidSize )
        , ( "fluidSimulationFrameRate", Encode.int settings.fluidSimulationFrameRate )
        , ( "diffusionIterations", Encode.int settings.diffusionIterations )
        , ( "pressureIterations", Encode.int settings.pressureIterations )
        , ( "colorScheme", encodeColorScheme settings.colorScheme )
        , ( "lineLength", Encode.float settings.lineLength )
        , ( "lineWidth", Encode.float settings.lineWidth )
        , ( "lineBeginOffset", Encode.float settings.lineBeginOffset )
        , ( "lineFadeOutLength", Encode.float settings.lineFadeOutLength )
        , ( "springStiffness", Encode.float settings.springStiffness )
        , ( "springVariance", Encode.float settings.springVariance )
        , ( "springMass", Encode.float settings.springMass )
        , ( "springDamping", Encode.float settings.springDamping )
        , ( "springRestLength", Encode.float settings.springRestLength )
        , ( "maxLineVelocity", Encode.float settings.maxLineVelocity )
        , ( "advectionDirection", encodeAdvectionDirection settings.advectionDirection )
        , ( "adjustAdvection", Encode.float settings.adjustAdvection )
        , ( "gridSpacing", Encode.int settings.gridSpacing )
        , ( "viewScale", Encode.float settings.viewScale )
        , ( "noiseChannel1", encodeNoise settings.noiseChannel1 )
        , ( "noiseChannel2", encodeNoise settings.noiseChannel2 )
        ]


encodeAdvectionDirection : AdvectionDirection -> Encode.Value
encodeAdvectionDirection =
    advectionDirectionToInt >> Encode.int


advectionDirectionToInt : AdvectionDirection -> Int
advectionDirectionToInt direction =
    case direction of
        Forward ->
            1

        Reverse ->
            -1


encodeColorScheme : ColorScheme -> Encode.Value
encodeColorScheme =
    colorSchemeToString >> Encode.string


colorSchemeToString : ColorScheme -> String
colorSchemeToString colorscheme =
    case colorscheme of
        Plasma ->
            "Plasma"

        Peacock ->
            "Peacock"

        Poolside ->
            "Poolside"

        Pollen ->
            "Pollen"


encodeBlendMethod : BlendMethod -> Encode.Value
encodeBlendMethod =
    blendMethodToString >> Encode.string


blendMethodToString : BlendMethod -> String
blendMethodToString blendMethod =
    case blendMethod of
        Curl ->
            "Curl"

        Wiggle ->
            "Wiggle"


encodeNoise : Noise -> Encode.Value
encodeNoise noise =
    Encode.object
        [ ( "scale", Encode.float noise.scale )
        , ( "multiplier", Encode.float noise.multiplier )
        , ( "offset1", Encode.float noise.offset1 )
        , ( "offset2", Encode.float noise.offset2 )
        , ( "offsetIncrement", Encode.float noise.offsetIncrement )
        , ( "delay", Encode.float noise.delay )
        , ( "blendDuration", Encode.float noise.blendDuration )
        , ( "blendThreshold", Encode.float noise.blendThreshold )
        , ( "blendMethod", encodeBlendMethod noise.blendMethod )
        ]
