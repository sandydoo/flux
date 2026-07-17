module Flux.Controls exposing
    ( all
    , appearance
    , colors
    , debug
    , fluid
    , noise
    , noiseChannels
    )

import Array
import Flux.Settings as Settings exposing (ColorMode(..), ColorPreset(..), Mode(..), Noise, NoiseChannelMsg(..), SettingMsg(..), Settings)
import FormatNumber as F
import FormatNumber.Locales as F
import Ui.Control as Control exposing (Control)
import Ui.Section as Section exposing (Section)


all : Settings -> List (Section SettingMsg)
all settings =
    [ colors settings
    , appearance settings
    , fluid settings
    , noise settings
    , noiseChannels settings
    , debug settings
    ]


colors : Settings -> Section SettingMsg
colors settings =
    Section.section
        { id = "colors"
        , title = "Colors"
        , controls =
            [ Control.gallery
                { id = "color-gallery"
                , label = "Color schemes"
                , items =
                    List.map (galleryItem settings.colorMode)
                        [ { id = "color-original"
                          , name = "Original"
                          , colorMode = Preset Original
                          , previewImage = "colors/original.png"
                          }
                        , { id = "color-plasma"
                          , name = "Plasma"
                          , colorMode = Preset Plasma
                          , previewImage = "colors/plasma.png"
                          }
                        , { id = "color-poolside"
                          , name = "Poolside"
                          , colorMode = Preset Poolside
                          , previewImage = "colors/poolside.png"
                          }
                        , { id = "color-gumdrop"
                          , name = "Gumdrop"
                          , colorMode = ImageFile "colors/gumdrop.png"
                          , previewImage = "colors/gumdrop.png"
                          }
                        , { id = "color-silver"
                          , name = "Silver"
                          , colorMode = ImageFile "colors/silver.png"
                          , previewImage = "colors/silver.png"
                          }
                        , { id = "color-freedom"
                          , name = "Freedom"
                          , colorMode = ImageFile "colors/freedom.png"
                          , previewImage = "colors/freedom.png"
                          }
                        ]
                }
            ]
        }


appearance : Settings -> Section SettingMsg
appearance settings =
    Section.section
        { id = "appearance"
        , title = "Appearance"
        , controls =
            [ Control.slider
                { id = "line-length"
                , title = "Line length"
                , description = "The maximum length of a line."
                , min = 1.0
                , max = 1000.0
                , step = 1.0
                , value = settings.lineLength
                , onInput = floatSetting SetLineLength
                , toString = formatFloat 0
                }
            , Control.slider
                { id = "line-width"
                , title = "Line width"
                , description = "The maximum width of a line."
                , min = 1.0
                , max = 20.0
                , step = 0.1
                , value = settings.lineWidth
                , onInput = floatSetting SetLineWidth
                , toString = formatFloat 1
                }
            , Control.slider
                { id = "line-fade-offset"
                , title = "Line fade offset"
                , description = "The point along a line where it begins to fade in."
                , min = 0.0
                , max = 1.0
                , step = 0.01
                , value = settings.lineBeginOffset
                , onInput = floatSetting SetLineBeginOffset
                , toString = formatFloat 2
                }
            , Control.slider
                { id = "variance"
                , title = "Variance"
                , description = "Less compels order. More wreaks chaos."
                , min = 0.0
                , max = 1.0
                , step = 0.01
                , value = settings.lineVariance
                , onInput = floatSetting SetLineVariance
                , toString = formatFloat 2
                }
            , Control.slider
                { id = "grid-spacing"
                , title = "Grid spacing"
                , description = "Adjust the spacing between the lines."
                , min = 1
                , max = 50
                , step = 1
                , value = settings.gridSpacing
                , onInput =
                    String.toInt
                        >> Maybe.withDefault Settings.default.gridSpacing
                        >> SetGridSpacing
                , toString = String.fromInt
                }
            ]
        }


fluid : Settings -> Section SettingMsg
fluid settings =
    Section.section
        { id = "fluid-simulation"
        , title = "Fluid simulation"
        , controls =
            [ Control.slider
                { id = "viscosity"
                , title = "Viscosity"
                , description = "A viscous fluid resists any change to its velocity. It spreads out and diffuses any force applied to it."
                , min = 0.1
                , max = 8.0
                , step = 0.1
                , value = settings.viscosity
                , onInput = floatSetting SetViscosity
                , toString = formatFloat 1
                }
            , Control.slider
                { id = "velocity-dissipation"
                , title = "Velocity dissipation"
                , description = "Velocity should decrease, or dissipate, as it travels through a fluid."
                , min = 0.0
                , max = 2.0
                , step = 0.1
                , value = settings.velocityDissipation
                , onInput = floatSetting SetVelocityDissipation
                , toString = formatFloat 1
                }
            , Control.slider
                { id = "diffusion-iterations"
                , title = "Diffusion iterations"
                , description = "Viscous fluids dissipate velocity through a process called “diffusion”. Each iteration enchances this effect and the diffusion strength is controlled by the fluid’s viscosity."
                , min = 0
                , max = 30
                , step = 1
                , value = settings.diffusionIterations
                , onInput = intSetting SetDiffusionIterations
                , toString = String.fromInt
                }
            , Control.slider
                { id = "pressure-iterations"
                , title = "Pressure iterations"
                , description = "Applying a force to the fluid creates pressure as the fluid pushes back. Calculating pressure is expensive, but the fluid will look unrealistic with fewer than 20 iterations."
                , min = 0
                , max = 60
                , step = 1
                , value = settings.pressureIterations
                , onInput = intSetting SetPressureIterations
                , toString = String.fromInt
                }
            ]
        }


noise : Settings -> Section SettingMsg
noise settings =
    Section.section
        { id = "noise"
        , title = "Noise"
        , controls =
            [ Control.slider
                { id = "noise-strength"
                , title = "Noise strength"
                , description = "Overall amount of velocity injected into the fluid."
                , min = 0.0
                , max = 4.0
                , step = 0.1
                , value = settings.noiseMultiplier
                , onInput = floatSetting SetNoiseMultiplier
                , toString = formatFloat 1
                }
            ]
        }


noiseChannels : Settings -> Section SettingMsg
noiseChannels settings =
    Section.section
        { id = "noise-channels"
        , title = "Noise channels"
        , controls =
            settings.noiseChannels
                |> Array.indexedMap noiseChannel
                |> Array.toList
        }


noiseChannel : Int -> Noise -> Control SettingMsg
noiseChannel index channel =
    let
        channelNumber =
            index + 1

        idPrefix =
            "noise-channel-" ++ String.fromInt channelNumber

        setChannel message =
            SetNoiseChannel index message
    in
    Control.group
        { id = idPrefix
        , title = "Channel " ++ String.fromInt channelNumber
        , controls =
            [ Control.slider
                { id = idPrefix ++ "-scale"
                , title = "Scale"
                , description = "The amount of detail in the noise. Larger values create more intricate patterns."
                , min = 0.1
                , max = 30.0
                , step = 0.1
                , value = channel.scale
                , onInput = floatSetting (SetNoiseChannelScale >> setChannel)
                , toString = formatFloat 1
                }
            , Control.slider
                { id = idPrefix ++ "-strength"
                , title = "Strength"
                , description = "The strength of the noise relative to the other channels."
                , min = 0.0
                , max = 1.0
                , step = 0.01
                , value = channel.multiplier
                , onInput = floatSetting (SetNoiseChannelMultiplier >> setChannel)
                , toString = formatFloat 2
                }
            , Control.slider
                { id = idPrefix ++ "-speed"
                , title = "Speed"
                , description = "How quickly the noise pattern changes."
                , min = 0
                , max = 100
                , step = 1
                , value = speedToSlider channel.offsetIncrement
                , onInput =
                    String.toInt
                        >> Maybe.withDefault 0
                        >> sliderToSpeed
                        >> SetNoiseChannelOffsetIncrement
                        >> setChannel
                , toString = String.fromInt
                }
            ]
        }


debug : Settings -> Section SettingMsg
debug settings =
    Section.section
        { id = "debug"
        , title = "Debug"
        , controls =
            [ Control.buttonGroup
                { id = "debug-mode"
                , label = "Debug mode"
                , options =
                    [ debugOption settings.mode "normal" "Normal" Normal
                    , debugOption settings.mode "noise" "Noise" DebugNoise
                    , debugOption settings.mode "fluid" "Fluid" DebugFluid
                    , debugOption settings.mode "pressure" "Pressure" DebugPressure
                    , debugOption settings.mode "divergence" "Divergence" DebugDivergence
                    ]
                }
            ]
        }


type alias GalleryItem =
    { id : String
    , name : String
    , colorMode : ColorMode
    , previewImage : String
    }


galleryItem : ColorMode -> GalleryItem -> Control.GalleryItem SettingMsg
galleryItem activeColor item =
    { id = item.id
    , label = item.name
    , imageSrc = item.previewImage
    , imageAlt = ""
    , active = item.colorMode == activeColor
    , onClick = SetColorMode item.colorMode
    }


debugOption : Mode -> String -> String -> Mode -> Control.ButtonOption SettingMsg
debugOption activeMode id label mode =
    { id = "debug-mode-" ++ id
    , label = label
    , active = activeMode == mode
    , onClick = SetMode mode
    }


floatSetting : (Float -> msg) -> String -> msg
floatSetting toMessage =
    String.toFloat >> Maybe.withDefault 0.0 >> toMessage


intSetting : (Int -> msg) -> String -> msg
intSetting toMessage =
    String.toInt >> Maybe.withDefault 0 >> toMessage


sliderToSpeed : Int -> Float
sliderToSpeed value =
    if value == 0 then
        0.0

    else
        0.5 * 2 ^ (toFloat (value - 100) / speedScale)


speedToSlider : Float -> Int
speedToSlider value =
    if value == 0.0 then
        0

    else
        100 + round (speedScale * logBase 2 (2.0 * value))


speedScale : Float
speedScale =
    7.0


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
