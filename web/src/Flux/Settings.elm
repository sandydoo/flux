module Flux.Settings exposing
    ( ColorMode(..)
    , ColorPreset(..)
    , Mode(..)
    , Noise
    , NoiseChannelMsg(..)
    , PressureMode(..)
    , SettingMsg(..)
    , Settings
    , default
    , encode
    , update
    )

import Array exposing (Array)
import Json.Encode as Encode


type alias Settings =
    { mode : Mode
    , seed : Maybe String
    , fluidSize : Int
    , fluidFrameRate : Int
    , fluidTimestep : Float
    , viscosity : Float
    , velocityDissipation : Float
    , pressureMode : PressureMode
    , diffusionIterations : Int
    , pressureIterations : Int
    , colorMode : ColorMode
    , lineLength : Float
    , lineWidth : Float
    , lineBeginOffset : Float
    , lineVariance : Float
    , gridSpacing : Int
    , viewScale : Float
    , noiseMultiplier : Float
    , noiseChannels : Array Noise
    }


type Mode
    = Normal
    | DebugNoise
    | DebugFluid
    | DebugPressure
    | DebugDivergence


type PressureMode
    = Retain
    | ClearWith Float


type ColorMode
    = Preset ColorPreset
    | ImageFile String


type ColorPreset
    = Original
    | Plasma
    | Poolside


type alias Noise =
    { scale : Float
    , multiplier : Float
    , offsetIncrement : Float
    }


default : Settings
default =
    { mode = Normal
    , seed = Nothing
    , fluidSize = 128
    , fluidFrameRate = 60
    , fluidTimestep = 1.0 / 60.0
    , viscosity = 5.0
    , velocityDissipation = 0.0
    , pressureMode = ClearWith 0.0
    , diffusionIterations = 3
    , pressureIterations = 19
    , colorMode = Preset Original
    , lineLength = 450.0
    , lineWidth = 9.0
    , lineBeginOffset = 0.4
    , lineVariance = 0.55
    , viewScale = 1.6
    , gridSpacing = 15
    , noiseMultiplier = 0.45
    , noiseChannels =
        Array.fromList
            [ { scale = 2.8
              , multiplier = 1.0
              , offsetIncrement = 0.001
              }
            , { scale = 15.0
              , multiplier = 0.7
              , offsetIncrement = 0.001 * 6.0
              }
            , { scale = 30.0
              , multiplier = 0.5
              , offsetIncrement = 0.001 * 12.0
              }
            ]
    }


type SettingMsg
    = SetMode Mode
    | SetViscosity Float
    | SetVelocityDissipation Float
    | SetPressureMode Float
    | SetDiffusionIterations Int
    | SetPressureIterations Int
    | SetColorMode ColorMode
    | SetLineLength Float
    | SetLineWidth Float
    | SetLineBeginOffset Float
    | SetLineVariance Float
    | SetGridSpacing Int
    | SetNoiseMultiplier Float
    | SetNoiseChannel Int NoiseChannelMsg


type NoiseChannelMsg
    = SetNoiseChannelScale Float
    | SetNoiseChannelMultiplier Float
    | SetNoiseChannelOffsetIncrement Float


update : SettingMsg -> Settings -> Settings
update msg settings =
    case msg of
        SetMode newMode ->
            { settings | mode = newMode }

        SetViscosity newViscosity ->
            { settings | viscosity = newViscosity }

        SetVelocityDissipation newVelocityDissipation ->
            { settings | velocityDissipation = newVelocityDissipation }

        SetPressureMode _ ->
            { settings | pressureMode = Retain }

        SetDiffusionIterations newDiffusionIterations ->
            { settings | diffusionIterations = newDiffusionIterations }

        SetPressureIterations newPressureIterations ->
            { settings | pressureIterations = newPressureIterations }

        SetColorMode newColorMode ->
            { settings | colorMode = newColorMode }

        SetLineLength newLineLength ->
            { settings | lineLength = newLineLength }

        SetLineWidth newLineWidth ->
            { settings | lineWidth = newLineWidth }

        SetLineBeginOffset newLineBeginOffset ->
            { settings | lineBeginOffset = newLineBeginOffset }

        SetLineVariance newLineVariance ->
            { settings | lineVariance = newLineVariance }

        SetGridSpacing newGridSpacing ->
            { settings | gridSpacing = newGridSpacing }

        SetNoiseMultiplier newNoiseMultiplier ->
            { settings | noiseMultiplier = newNoiseMultiplier }

        SetNoiseChannel channelNumber noiseMsg ->
            case Array.get channelNumber settings.noiseChannels of
                Just channel ->
                    { settings
                        | noiseChannels =
                            Array.set channelNumber
                                (updateNoiseChannel noiseMsg channel)
                                settings.noiseChannels
                    }

                Nothing ->
                    settings


updateNoiseChannel : NoiseChannelMsg -> Noise -> Noise
updateNoiseChannel msg noise =
    case msg of
        SetNoiseChannelScale newScale ->
            { noise | scale = newScale }

        SetNoiseChannelMultiplier newMultiplier ->
            { noise | multiplier = newMultiplier }

        SetNoiseChannelOffsetIncrement newOffsetIncrement ->
            { noise | offsetIncrement = newOffsetIncrement }


{-| Encode settings using the externally tagged enum representation expected by
Serde.
-}
encode : Settings -> Encode.Value
encode settings =
    Encode.object
        [ ( "mode", encodeMode settings.mode )
        , ( "seed", settings.seed |> Maybe.map Encode.string |> Maybe.withDefault Encode.null )
        , ( "fluidSize", Encode.int settings.fluidSize )
        , ( "fluidFrameRate", Encode.int settings.fluidFrameRate )
        , ( "fluidTimestep", Encode.float settings.fluidTimestep )
        , ( "viscosity", Encode.float settings.viscosity )
        , ( "velocityDissipation", Encode.float settings.velocityDissipation )
        , ( "pressureMode", encodePressureMode settings.pressureMode )
        , ( "diffusionIterations", Encode.int settings.diffusionIterations )
        , ( "pressureIterations", Encode.int settings.pressureIterations )
        , ( "colorMode", encodeColorMode settings.colorMode )
        , ( "lineLength", Encode.float settings.lineLength )
        , ( "lineWidth", Encode.float settings.lineWidth )
        , ( "lineBeginOffset", Encode.float settings.lineBeginOffset )
        , ( "lineVariance", Encode.float settings.lineVariance )
        , ( "gridSpacing", Encode.int settings.gridSpacing )
        , ( "viewScale", Encode.float settings.viewScale )
        , ( "noiseMultiplier", Encode.float settings.noiseMultiplier )
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


encodePressureMode : PressureMode -> Encode.Value
encodePressureMode pressureMode =
    case pressureMode of
        Retain ->
            Encode.string "Retain"

        ClearWith pressure ->
            Encode.object [ ( "ClearWith", Encode.float pressure ) ]


encodeColorMode : ColorMode -> Encode.Value
encodeColorMode colorMode =
    case colorMode of
        Preset preset ->
            Encode.object
                [ ( "Preset", encodeColorPreset preset )
                ]

        ImageFile path ->
            Encode.object
                [ ( "ImageFile", Encode.string path )
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


encodeNoise : Noise -> Encode.Value
encodeNoise noise =
    Encode.object
        [ ( "scale", Encode.float noise.scale )
        , ( "multiplier", Encode.float noise.multiplier )
        , ( "offsetIncrement", Encode.float noise.offsetIncrement )
        ]
