port module Main exposing (..)

import Array exposing (Array)
import Browser
import Browser.Events as BrowserEvent
import FormatNumber as F
import FormatNumber.Locales as F
import Html exposing (Html)
import Html.Attributes as HA
import Html.Events as Event
import Json.Decode as Decode exposing (Decoder)
import Json.Decode.Pipeline as Decode
import Json.Encode as Encode
import Set exposing (Set)



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
    { mode : Mode
    , fluidSize : Int
    , fluidFrameRate : Int
    , fluidTimestep : Float
    , viscosity : Float
    , velocityDissipation : Float
    , clearPressure : ClearPressure
    , diffusionIterations : Int
    , pressureIterations : Int
    , colorMode : ColorMode
    , lineLength : Float
    , lineWidth : Float
    , lineBeginOffset : Float
    , lineVariance : Float
    , gridSpacing : Int
    , viewScale : Float
    , noiseChannels : Array Noise
    }


type Mode
    = Normal
    | DebugNoise
    | DebugFluid
    | DebugPressure
    | DebugDivergence


type ClearPressure
    = KeepPressure
    | ClearPressure Float


type ColorMode
    = Preset ColorPreset


type ColorPreset
    = Original
    | Plasma
    | Poolside
    | Freedom


type alias Noise =
    { scale : Float
    , multiplier : Float
    , offsetIncrement : Float
    }


defaultSettings : Settings
defaultSettings =
    { mode = Normal
    , fluidSize = 128
    , fluidFrameRate = 60
    , fluidTimestep = 1.0 / 60.0
    , viscosity = 5.0
    , velocityDissipation = 0.0
    , clearPressure = KeepPressure
    , diffusionIterations = 3
    , pressureIterations = 19
    , colorMode = Preset Original
    , lineLength = 550.0
    , lineWidth = 10.0
    , lineBeginOffset = 0.4
    , lineVariance = 0.45
    , viewScale = 1.6
    , gridSpacing = 15
    , noiseChannels =
        Array.fromList
            [ { scale = 2.5
              , multiplier = 1.0
              , offsetIncrement = 0.0015
              }
            , { scale = 15.0
              , multiplier = 0.7
              , offsetIncrement = 0.0015 * 6.0
              }
            , { scale = 30.0
              , multiplier = 0.5
              , offsetIncrement = 0.0015 * 12.0
              }
            ]
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
    = SetMode Mode
    | SetViscosity Float
    | SetVelocityDissipation Float
    | SetClearPressure Float
    | SetDiffusionIterations Int
    | SetPressureIterations Int
    | SetColorPreset ColorPreset
    | SetLineLength Float
    | SetLineWidth Float
    | SetLineBeginOffset Float
    | SetLineVariance Float
    | SetNoiseChannel Int NoiseMsg


type NoiseMsg
    = SetNoiseScale Float
    | SetNoiseMultiplier Float
    | SetNoiseOffsetIncrement Float


updateSettings : SettingMsg -> Settings -> Settings
updateSettings msg settings =
    case msg of
        SetMode newMode ->
            { settings | mode = newMode }

        SetViscosity newViscosity ->
            { settings | viscosity = newViscosity }

        SetVelocityDissipation newVelocityDissipation ->
            { settings | velocityDissipation = newVelocityDissipation }

        SetClearPressure _ ->
            { settings | clearPressure = KeepPressure }

        SetDiffusionIterations newDiffusionIterations ->
            { settings | diffusionIterations = newDiffusionIterations }

        SetPressureIterations newPressureIterations ->
            { settings | pressureIterations = newPressureIterations }

        SetColorPreset newColorPreset ->
            { settings | colorMode = Preset newColorPreset }

        SetLineLength newLineLength ->
            { settings | lineLength = newLineLength }

        SetLineWidth newLineWidth ->
            { settings | lineWidth = newLineWidth }

        SetLineBeginOffset newLineBeginOffset ->
            { settings | lineBeginOffset = newLineBeginOffset }

        SetLineVariance newLineVariance ->
            { settings | lineVariance = newLineVariance }

        SetNoiseChannel channelNumber noiseMsg ->
            let
                maybeChannel =
                    Array.get channelNumber settings.noiseChannels
            in
            case maybeChannel of
                Just channel ->
                    { settings | noiseChannels = Array.set channelNumber (updateNoise noiseMsg channel) settings.noiseChannels }

                Nothing ->
                    settings


updateNoise : NoiseMsg -> Noise -> Noise
updateNoise msg noise =
    case msg of
        SetNoiseScale newScale ->
            { noise | scale = newScale }

        SetNoiseMultiplier newMultiplier ->
            { noise | multiplier = newMultiplier }

        SetNoiseOffsetIncrement newOffsetIncrement ->
            { noise | offsetIncrement = newOffsetIncrement }



-- SUBSCRIPTIONS


subscriptions : Model -> Sub Msg
subscriptions { isOpen } =
    BrowserEvent.onKeyDown
        (Decode.field "key" Decode.string
            |> Decode.andThen (toggleControlsOnKey isOpen)
        )


keysThatOpenControls : Set String
keysThatOpenControls =
    Set.fromList [ "c" ]


keysThatCloseControls : Set String
keysThatCloseControls =
    Set.insert "escape" keysThatOpenControls


toggleControlsOnKey : Bool -> String -> Decoder Msg
toggleControlsOnKey isOpen key =
    let
        activeKeys =
            if isOpen then
                keysThatCloseControls

            else
                keysThatOpenControls
    in
    if Set.member (String.toLower key) activeKeys then
        Decode.succeed ToggleControls

    else
        Decode.fail ""



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
                        [ Html.text "© 2022 Sander Melnikov" ]
                    ]
                , Html.li []
                    [ Html.a
                        [ HA.href "https://twitter.com/sandy_doo/" ]
                        [ Html.text "Follow me on Twitter" ]
                    ]
                , Html.li []
                    [ Html.a
                        [ HA.href "https://sandydoo.gumroad.com/l/flux" ]
                        [ Html.text "Buy this screensaver" ]
                    ]
                ]
            ]
        ]


viewSettings : Settings -> Html Msg
viewSettings settings =
    let
        whenColorModeIsPreset colorMode f =
            case colorMode of
                Preset colorPreset ->
                    f colorPreset
    in
    Html.ul
        [ HA.class "control-list" ]
    <|
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
        , whenColorModeIsPreset settings.colorMode <|
            \colorPreset ->
                viewButtonGroup (SetColorPreset >> SaveSetting)
                    colorPreset
                    [ ( "Original", Original )
                    , ( "Plasma", Plasma )
                    , ( "Poolside", Poolside )
                    , ( "🇺🇦", Freedom )
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
                    , max = 1000.0
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
                The point along a line where it begins to fade in.
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
            Control
                "Variance"
                """
                Less compels order. More wreaks chaos.
                """
                (Slider
                    { min = 0.0
                    , max = 1.0
                    , step = 0.01
                    , value = settings.lineVariance
                    , onInput =
                        \value ->
                            String.toFloat value
                                |> Maybe.withDefault 0.0
                                |> SetLineVariance
                                |> SaveSetting
                    , toString = formatFloat 2
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
                    , max = 8.0
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
                Applying a force to the fluid creates pressure as the fluid pushes back.
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
        ]
            ++ (Array.toList <|
                    Array.indexedMap
                        (\index channel ->
                            let
                                title =
                                    "Channel " ++ String.fromInt (index + 1)
                            in
                            viewNoiseChannel title (SetNoiseChannel index) channel
                        )
                        settings.noiseChannels
               )
            ++ viewDebug settings.mode


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


viewNoiseChannel : String -> (NoiseMsg -> SettingMsg) -> Noise -> Html Msg
viewNoiseChannel title setNoiseChannel noiseChannel =
    Html.div
        [ HA.class "control-list-single" ]
        [ Html.div []
            [ Html.h4 [] [ Html.text title ]
            ]
        , viewControl <|
            Control
                "Scale"
                "The amount of detail in the noise. Larger values create more intricate patterns."
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
                "The amount of force applied by the noise."
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
            let
                -- Use this to stretch out the log scale a bit
                scale : Float
                scale =
                    7.0

                toSpeed : Int -> Float
                toSpeed n =
                    if n == 0 then
                        0.0

                    else
                        0.5 * 2 ^ (toFloat (n - 100) / scale)

                fromSpeed : Float -> Int
                fromSpeed n =
                    if n == 0.0 then
                        0

                    else
                        100 + round (scale * logBase 2 (2.0 * n))
            in
            -- This scale is logarithmic. I should probably refactor the other
            -- sliders to 0-100 as well.
            Control
                "Speed"
                "How quickly the noise pattern changes."
                (Slider
                    { min = 0
                    , max = 100
                    , step = 1
                    , value = fromSpeed noiseChannel.offsetIncrement
                    , onInput =
                        \value ->
                            String.toInt value
                                |> Maybe.withDefault 0
                                |> toSpeed
                                |> SetNoiseOffsetIncrement
                                |> setNoiseChannel
                                |> SaveSetting
                    , toString = String.fromInt
                    }
                )
        ]


viewDebug : Mode -> List (Html Msg)
viewDebug mode =
    [ Html.h2 [ HA.class "col-span-2-md" ] [ Html.text "Debug" ]
    , viewButtonGroup (SetMode >> SaveSetting)
        mode
        [ ( "Normal", Normal )
        , ( "Noise", DebugNoise )
        , ( "Fluid", DebugFluid )

        --, ( "Pressure", DebugPressure )
        --, ( "Divergence", DebugDivergence )
        ]
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
        , thousandSeparator = ""
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
-- For enums containing data, use the externally tagged representation.
-- https://serde.rs/enum-representations.html#externally-tagged


encodeSettings : Settings -> Encode.Value
encodeSettings settings =
    Encode.object
        [ ( "mode", encodeMode settings.mode )
        , ( "fluidSize", Encode.int settings.fluidSize )
        , ( "fluidFrameRate", Encode.int settings.fluidFrameRate )
        , ( "fluidTimestep", Encode.float settings.fluidTimestep )
        , ( "viscosity", Encode.float settings.viscosity )
        , ( "velocityDissipation", Encode.float settings.velocityDissipation )
        , ( "clearPressure", encodeClearPressure settings.clearPressure )
        , ( "diffusionIterations", Encode.int settings.diffusionIterations )
        , ( "pressureIterations", Encode.int settings.pressureIterations )
        , ( "colorMode", encodeColorMode settings.colorMode )
        , ( "lineLength", Encode.float settings.lineLength )
        , ( "lineWidth", Encode.float settings.lineWidth )
        , ( "lineBeginOffset", Encode.float settings.lineBeginOffset )
        , ( "lineVariance", Encode.float settings.lineVariance )
        , ( "gridSpacing", Encode.int settings.gridSpacing )
        , ( "viewScale", Encode.float settings.viewScale )
        , ( "noiseChannels", Encode.array encodeNoise settings.noiseChannels )
        ]


encodeMode : Mode -> Encode.Value
encodeMode mode =
    Encode.string <|
        case mode of
            Normal ->
                "Normal"

            DebugNoise ->
                "DebugNoise"

            DebugFluid ->
                "DebugFluid"

            DebugPressure ->
                "DebugPressure"

            DebugDivergence ->
                "DebugDivergence"


encodeClearPressure : ClearPressure -> Encode.Value
encodeClearPressure clearPressure =
    case clearPressure of
        KeepPressure ->
            Encode.string "KeepPressure"

        ClearPressure pressure ->
            Encode.object [ ( "ClearPressure", Encode.float pressure ) ]


encodeColorMode : ColorMode -> Encode.Value
encodeColorMode colorMode =
    case colorMode of
        Preset preset ->
            Encode.object
                [ ( "Preset", encodeColorPreset preset )
                ]


encodeColorPreset : ColorPreset -> Encode.Value
encodeColorPreset =
    colorPresetToString >> Encode.string


colorPresetToString : ColorPreset -> String
colorPresetToString colorPreset =
    case colorPreset of
        Original ->
            "Original"

        Plasma ->
            "Plasma"

        Poolside ->
            "Poolside"

        Freedom ->
            "Freedom"


encodeNoise : Noise -> Encode.Value
encodeNoise noise =
    Encode.object
        [ ( "scale", Encode.float noise.scale )
        , ( "multiplier", Encode.float noise.multiplier )
        , ( "offsetIncrement", Encode.float noise.offsetIncrement )
        ]
