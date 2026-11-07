Inari: Innovation Artificial Intelligence
==========================================

An unofficial implementation of Innovation Ultimate by Carl Chudyk, with a neural network-based AI opponent.

The .exe file is unsigned, so Windows will warn about it; if you don't trust it look at the source yourself and build it.

Elo ratings are rough approximations based on AI playing itself. Lower ratings use the policy network
sampled at some temperature (higher temperatures are more random and weaker), where Elo 0 is a fully random opponent.
Higher ratings do Monte Carlo Tree search - more iterations give a tougher opponent.

UI mostly self-explanatory if you're familiar with the game except:
-Right click to zoom on a Card
-Left click or hover over piles to see details
-F11 for full-screen mode

The app automatically saves every move of every game to a "saves" subdirectory.

Licenses
--------
Inari is MIT licensed. Card texts are (c) Asmadi Games; this project is not
affiliated with or endorsed by Asmadi Games. Fonts: Cinzel and Alegreya Sans,
under the SIL Open Font License.

Source: https://github.com/convexish/inari
