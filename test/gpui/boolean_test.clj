(ns gpui.boolean-test
  (:require [clojure.data.json :as json]
            [clojure.string :as str]
            [clojure.test :refer [deftest is testing]]
            [gpui.runtime :as runtime]
            [gpui.ui :as ui]))

(defn- schema-fields [struct rust-type]
  (let [source (slurp "host/src/protocol.rs")
        body (second (re-find (re-pattern (str "(?s)pub struct " struct " \\{(.*?)\n\\}")) source))]
    (into #{}
          (map (fn [[_ field]] (keyword (str/replace field "_" "-"))))
          (re-seq (re-pattern (str "pub (\\w+): " (java.util.regex.Pattern/quote rust-type) ",")) body))))

(defn- wire [node]
  ;; Exercise the public construction -> export -> actual JSON path.
  (json/read-str (json/write-str (runtime/export-tree node)) :key-fn keyword))

(defn- error-text [node]
  (->> (tree-seq :children :children (wire node))
       (keep :text)
       (str/join "\n")))

(deftest boolean-categories-match-the-native-schema
  (is (= (conj (schema-fields "Node" "bool") :caret)
         @#'runtime/node-boolean-flags))
  (is (= (schema-fields "Node" "Option<bool>") @#'runtime/node-boolean-overrides))
  (is (= (schema-fields "Item" "bool") @#'runtime/item-boolean-flags))
  (is (= (schema-fields "Item" "Option<bool>") @#'runtime/item-boolean-overrides))
  (is (= (schema-fields "StyledKeys" "Option<bool>") @#'runtime/styled-boolean-overrides))
  (is (= #{:shadow} (schema-fields "ButtonCustomVariantSpec" "Option<bool>"))))

(deftest every-schema-boolean-preserves-omission-and-boolean-values
  (doseq [[flags overrides build path]
          [[(conj (schema-fields "Node" "bool") :caret)
            (schema-fields "Node" "Option<bool>")
            #(merge {:type :button} %) []]
           [(schema-fields "Item" "bool") (schema-fields "Item" "Option<bool>")
            #(assoc (ui/select nil) :options [(merge {:id "item"} %)]) [:options 0]]]
          [keys nullable?] [[flags false] [overrides true]]
          k keys]
    (testing (str path " " k)
      (is (not (contains? (get-in (wire (build {})) path) k)))
      (doseq [value [nil false true]]
        (let [result (get-in (wire (build {k value})) path)]
          (is (contains? result k))
          (is (= (if (and (nil? value) (not nullable?)) false value) (get result k))))))))

(deftest all-boolean-fields-reject-truthy-values-with-a-field-path
  (doseq [k (into (conj (schema-fields "Node" "bool") :caret)
                  (schema-fields "Node" "Option<bool>"))]
    (is (str/includes? (error-text {:type :button k "true"})
                       (str "invalid UI boolean at " (name k) ":"))))
  (doseq [[value type] [["false" "string"] [:selected "keyword"] [0 "number"]
                        [{} "map"] [(fn [] true) "function"]]]
    (doseq [k [:disabled :focus-ring]]
      (let [text (error-text (ui/window (ui/button "Action" {k value})))]
        (is (str/includes? text (str "children[0]." (name k))))
        (is (str/includes? text (str "expected true, false, or nil, got " type))))))
  (is (empty? @@#'runtime/callbacks)
      "an invalid function-valued boolean is not registered as a callback"))

(deftest typed-nested-locations-use-their-own-schema
  (let [node {:disabled nil :focus-ring nil}
        item {:disabled nil :checked nil
              :items [{:disabled nil}]
              :content node :children [node]
              :style {:shadow nil} :label-style {:strikethrough nil}
              :cells [node [{:disabled nil}]]}
        node-slots [:trigger :footer :stack-style :shimmer-style :separator-style
                    :content-style :list-style :row-style :jump-button-style :jump-button-renderer]
        tree (merge {:type :window
                     :children [node] :left [node] :right [node]
                     :items [item] :options [item] :links [item] :series [item]
                     :header-groups [[item]]}
                    (zipmap node-slots (repeat node)))
        result (wire tree)]
    (doseq [p (concat (map vector node-slots)
                      [[:children 0] [:left 0] [:right 0]
                       [:items 0 :content] [:items 0 :children 0] [:items 0 :cells 0]])]
      (is (false? (get-in result (conj p :disabled))) (str p))
      (is (contains? (get-in result p) :focus-ring))
      (is (nil? (get-in result (conj p :focus-ring)))))
    (doseq [p [[:items 0] [:options 0] [:links 0] [:series 0] [:header-groups 0 0]]]
      (is (false? (get-in result (conj p :disabled))))
      (is (false? (get-in result (into p [:items 0 :disabled]))))
      (is (nil? (get-in result (conj p :checked)))))
    (is (false? (get-in result [:items 0 :style :shadow])))
    (is (false? (get-in result [:items 0 :label-style :strikethrough])))
    (is (= [{:disabled nil}] (get-in result [:items 0 :cells 1]))
        "array-valued table cells are data, while object cells are Nodes")))

(deftest nested-type-errors-are-actionable
  (doseq [[tree location]
          [[(ui/select nil {:options [{:label "Group" :items [{:id :a :disabled :wrong}]}]})
            "options[0].items[0].disabled"]
           [{:type :data-table :header-groups [[{:resizable "true"}]]}
            "header-groups[0][0].resizable"]
           [{:type :sidebar :items [{:label-style {:shadow (fn [])}}]}
            "items[0].label-style.shadow"]
           [(ui/button "Custom" {:custom-variant {:shadow :wrong}})
            "custom-variant.shadow"]
           [(ui/nav-stack {:item {:match [{:shadow "false"}]}})
            "item.match[0].shadow"]]]
    (is (str/includes? (error-text tree) (str "invalid UI boolean at " location ":")))))

(deftest payloads-callback-data-and-nullable-recipes-stay-untouched
  (let [payload {:type "button" :disabled nil :selected nil :loading nil
                 :children [{:disabled nil}] :unknown {:shadow nil}}
        recipe {:shadow nil :truncate nil :overflow-hidden nil
                :left {:from 0 :to 1 :disabled nil}
                :match [{:strikethrough nil}]}
        received (atom nil)
        tree {:type :window :value payload :metadata payload :item recipe
              :custom-variant {:shadow nil}
              :items [{:value payload :values [payload] :fill payload}]
              :on-change #(reset! received %)}
        result (wire tree)]
    (doseq [p [[:value] [:metadata] [:items 0 :value] [:items 0 :values 0] [:items 0 :fill]]]
      (is (= payload (get-in result p))))
    (is (= recipe (:item result)))
    (is (= {:shadow nil} (:custom-variant result)))
    (runtime/invoke-callback! (:on-change result) payload true)
    (is (= payload @received))))

(deftest public-constructors-retain-boolean-types-and-defaults
  (testing "ordinary button flags and aliases"
    (let [node (wire (ui/button "Action" {:disabled nil :loading nil :selected nil :caret nil}))]
      (is (= [false false false false]
             (mapv node [:disabled :loading :selected :dropdown-caret])))
      (is (not (contains? node :caret)))))
  (testing "items no longer drop invalid flags or coerce optional checked"
    (doseq [constructor [ui/option-item ui/tree-item ui/menu-item]]
      (let [result (constructor {:id :a :disabled nil :expanded nil :separator nil :checked nil})]
        (is (= [false false false nil]
               (mapv result [:disabled :expanded :separator :checked]))))
      (is (str/includes? (error-text {:type :tree :items [(constructor {:id :a :disabled :wrong})]})
                         "items[0].disabled"))
      (is (str/includes? (error-text {:type :tree :items [(constructor {:id :a :checked "true"})]})
                         "items[0].checked"))))
  (is (str/includes? (error-text {:type :native-menu
                                  :items [(ui/menu-item {:separator true :disabled :wrong})]})
                     "items[0].disabled"))
  (testing "nullable column overrides retain nil and reject invalid values"
    (let [table (wire (ui/data-table {:columns [{:id :a :resizable nil :movable nil :selectable nil}]
                                      :rows []}))]
      (is (= {:resizable nil :movable nil :selectable nil}
             (select-keys (get-in table [:options 0]) [:resizable :movable :selectable]))))
    (is (str/includes? (error-text (ui/data-table {:columns [{:id :a :resizable :wrong}] :rows []}))
                       "options[0].resizable")))
  (testing "default-true searchable controls distinguish omission from explicit nil"
    (doseq [constructor [#(ui/combobox nil %) #(ui/command [] %)]]
      (is (true? (:searchable (wire (constructor {})))))
      (is (false? (:searchable (wire (constructor {:searchable nil})))))
      (is (str/includes? (error-text (constructor {:searchable :wrong})) "searchable"))))
  (testing "selected IDs are values rather than boolean flags"
    (let [result (wire (ui/command [{:id :save}] {:selected :save}))]
      (is (= "save" (:value result)))
      (is (not (contains? result :selected)))))
  (testing "nullable native defaults are not converted into explicit false"
    (let [node (wire (ui/input "text" {:bordered nil :focus-ring nil :auto-close nil :smart-indent nil}))]
      (is (= {:bordered nil :focus-ring nil :auto-close nil :smart-indent nil}
             (select-keys node [:bordered :focus-ring :auto-close :smart-indent]))))))

(deftest controlled-state-shorthands-are-nil-aware-but-not-truthy
  (doseq [constructor [#(ui/checkbox % (fn [])) ui/switch ui/toggle]]
    (doseq [state [nil false true]]
      (is (= (boolean state) (:checked (wire (constructor state))))))
    (is (str/includes? (error-text (constructor :truthy)) "checked")))
  (doseq [constructor [ui/dialog ui/alert-dialog ui/sheet]]
    (is (false? (:open (wire (constructor nil)))))
    (is (false? (:open (wire (constructor {:open? nil})))))
    (is (nil? (:open (wire (constructor {:open nil})))))
    (is (false? (:open (wire (constructor {:open? false :open true})))))
    (is (true? (:open (wire (constructor true)))))
    (is (str/includes? (error-text (constructor :truthy)) "open"))
    (is (str/includes? (error-text (constructor {:open? :truthy})) "open")))
  (is (false? (:open (wire (ui/popover nil)))))
  (is (str/includes? (error-text (ui/popover :truthy)) "open"))
  (is (false? (:open (wire (ui/notification {:open? nil})))))
  (is (nil? (:open (wire (ui/notification {:open nil})))))
  (is (str/includes? (error-text (ui/notification {:open? :truthy})) "open")))
